//! The cursor resolution of the controller: the pick of a pointer position under the running
//! object snap, tracking, ortho, polar and the snap step. The object snap is found through a
//! [`SnapIndex`] that is kept for one document revision and rebuilt only when the revision or the
//! space changes; tracking points are acquired when the cursor hovers an object snap point.

use arc_base::math::{Point2, Vector2};
use arc_base::{ObjectId, Result, Tolerance};
use arc_doc::Snapshot;
use arc_draft::snap::{
    ConstraintSource, Frame, MAX_ACQUIRED, SnapIndex, SnapKind, SnapModes, SnapQuery, track,
};

use super::{MAX_TRACKING_PATHS, TrackOut};
use crate::draw::create::{from_meters, meters_per_unit};
use crate::precision::resolve::{self, Request};
use crate::precision::snaps::Override;
use crate::settings::store::Values;

/// The resolved cursor, in meters.
#[derive(Debug, Clone)]
pub(super) struct Resolved {
    /// The point a pick gives.
    pub(super) point: [f64; 2],
    /// What decided it.
    pub(super) source: ConstraintSource,
    /// The object snap kind, when an object snap decided it.
    pub(super) snap: Option<SnapKind>,
    /// The tracking paths through the acquired points that the cursor is near.
    pub(super) tracking: Vec<TrackOut>,
}

/// The snap index of one revision and space.
struct Cached {
    revision: u64,
    space: ObjectId,
    index: SnapIndex,
}

/// The index, the acquired tracking points and the count of index builds.
#[derive(Default)]
pub(super) struct Resolver {
    cache: Option<Cached>,
    builds: u64,
    /// Acquired points in meters, oldest first.
    acquired: Vec<[f64; 2]>,
    /// The temporary tracking origin of the next point, in meters, when a `TEMPTRACK` waits for it:
    /// the only origin the tracking paths run from.
    only: Option<[f64; 2]>,
}

impl Resolver {
    /// The number of snap indexes built so far.
    pub(super) fn builds(&self) -> u64 {
        self.builds
    }

    /// Forgets the acquired tracking points (at the end of a command).
    pub(super) fn clear_acquired(&mut self) {
        self.acquired.clear();
        self.only = None;
    }

    /// Sets the temporary tracking origin of the next point (meters), or none.
    pub(super) fn set_only(&mut self, origin: Option<[f64; 2]>) {
        self.only = origin;
    }

    /// The index of `space` at the revision of `snapshot`, built now when the cached one is for
    /// another revision or space.
    fn index(&mut self, snapshot: &Snapshot, space: ObjectId) -> Result<&SnapIndex> {
        let revision = snapshot.revision();
        let fresh = matches!(&self.cache, Some(c) if c.revision == revision && c.space == space);
        if !fresh {
            // The bounds and tolerance of the query are the defaults of every query of the
            // controller, so the index serves all of them.
            let query = SnapQuery::new(
                Point2::new(0.0, 0.0),
                1.0,
                SnapModes::ALL,
                Tolerance::DEFAULT,
            );
            let index = SnapIndex::build(snapshot, space, &query)?;
            self.builds += 1;
            self.cache = Some(Cached {
                revision,
                space,
                index,
            });
        }
        match &self.cache {
            Some(c) => Ok(&c.index),
            // Unreachable: the cache was filled above. Reported rather than panicking.
            None => Err(arc_base::Error::new(
                arc_draft::error::codes::SNAP_INDEX_STALE,
            )),
        }
    }

    /// Resolves the pointer at `at` (meters) with the pick aperture `aperture` (meters), from the
    /// last point `base` (meters) when there is one. Hovering an object snap point acquires it.
    ///
    /// # Errors
    ///
    /// The errors of the snap index, the tracking and the constraints.
    pub(super) fn resolve(
        &mut self,
        snapshot: &Snapshot,
        space: ObjectId,
        values: &Values,
        at: [f64; 2],
        aperture: f64,
        base: Option<[f64; 2]>,
    ) -> Result<Resolved> {
        let cursor = [from_meters(snapshot, at[0]), from_meters(snapshot, at[1])];
        let aperture = from_meters(snapshot, aperture);
        let from = base.map(|b| [from_meters(snapshot, b[0]), from_meters(snapshot, b[1])]);
        let modes = resolve::running_set(values, Override::None);
        let ucs = crate::precision::ucs::current(snapshot);
        let ucs_x = Vector2::new(ucs.x_axis[0], ucs.x_axis[1]);
        let frame = Frame::new(ucs_x)?;
        let polar = resolve::polar(values)?;

        let snapped = if modes == SnapModes::NONE || aperture <= 0.0 {
            None
        } else {
            let index = self.index(snapshot, space)?;
            let request = Request {
                snapshot,
                space,
                cursor,
                aperture,
                from,
                tracked: None,
                over: Override::None,
            };
            resolve::object_snap_indexed(&request, modes, index)?
        };

        // Tracking runs on the points acquired so far, and only while no object snap decides.
        let mut tracking = Vec::new();
        let mut tracked = None;
        let origins: &[[f64; 2]] = match self.only.as_ref() {
            Some(origin) => std::slice::from_ref(origin),
            None => self.acquired.as_slice(),
        };
        if snapped.is_none() && !origins.is_empty() {
            let acquired: Vec<Point2> = origins
                .iter()
                .map(|p| Point2::new(from_meters(snapshot, p[0]), from_meters(snapshot, p[1])))
                .collect();
            let hit = track(
                Point2::new(cursor[0], cursor[1]),
                &acquired,
                &frame,
                polar.as_ref(),
                aperture,
                &Tolerance::DEFAULT,
            )?;
            if let Some(hit) = hit {
                tracked = Some([hit.point.x, hit.point.y]);
                let mpu = meters_per_unit(snapshot);
                tracking = hit
                    .paths
                    .iter()
                    .take(MAX_TRACKING_PATHS)
                    .map(|path| TrackOut {
                        origin: [path.origin.x * mpu, path.origin.y * mpu],
                        direction: [path.direction.x, path.direction.y],
                    })
                    .collect();
            }
        }

        let request = Request {
            snapshot,
            space,
            cursor,
            aperture,
            from,
            tracked,
            over: Override::None,
        };
        let kind = snapped.as_ref().map(|(kind, _)| *kind);
        let pick = resolve::resolve_snapped(&request, values, snapped)?;
        let mpu = meters_per_unit(snapshot);
        let point = [pick.point[0] * mpu, pick.point[1] * mpu];
        if pick.source == ConstraintSource::ObjectSnap {
            self.acquire(point);
        }
        Ok(Resolved {
            point,
            source: pick.source,
            snap: kind,
            tracking,
        })
    }

    /// Acquires a tracking point (meters) unless it is already acquired or the list is full.
    fn acquire(&mut self, point: [f64; 2]) {
        let known = self.acquired.iter().any(|p| p == &point);
        if !known && self.acquired.len() < MAX_ACQUIRED {
            self.acquired.push(point);
        }
    }
}
