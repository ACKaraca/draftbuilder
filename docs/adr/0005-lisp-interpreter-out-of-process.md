# ADR-0005: The Lisp-dialect interpreter runs out of process as an automation client

Existing user routines run in a Rust interpreter hosted in a separate process that talks to
DraftBuilder through the Arc Automation Protocol.

- **Status:** accepted
- **Suite context:** [suite decision records](https://github.com/ACKaraca/arc-suite/tree/main/docs/adr)

## Context

Drafting teams depend on large libraries of routines in the Lisp dialect common to DWG
applications. The suite decides that scripting runs out of process as a client of the automation
protocol and that no interpreter is embedded in the first releases. Routines are interactive (they
prompt for points and options), access entities through group-code lists in tight loops, and
sometimes misbehave (infinite loops, unbounded allocation).

## Decision

- `draftbuilder-lisp` implements the reader, evaluator and standard library in Rust;
  `apps/draftbuilder-lisp` hosts it in a separate process per application session, with one
  namespace per drawing.
- The interpreter is an automation-protocol client. Commands it defines are registered in the command
  registry; input functions request prompts from the active prompt machine; `command` calls send
  tokens; entity and selection functions use batched protocol calls.
- DraftBuilder applies the routine's changes as normal commands inside transactions; stopping or
  crashing the host rolls back the partial transaction and never affects the open drawing.
- Dialog definition files are rendered by the native UI from a declarative description sent by the
  host.

## Consequences

- One automation surface is secured and documented for Lisp, Python, plug-ins and the assistant.
- Protocol round trips add latency; batching and caching of read-only data are required to meet the
  target in [overview.md](../architecture/overview.md).
- Event callbacks are asynchronous; routines that rely on synchronous callback side effects need
  adaptation, which is documented per callback.

## Alternatives considered

- **Embedding the interpreter in the application process.** Fastest, but contradicts the suite
  decision and couples routine faults to the drawing.
- **Translating routines to Python.** Breaks existing libraries and their maintainers' workflow.
- **Running the interpreter as a WebAssembly plug-in.** A plausible later host; it would use the same
  command and protocol surface.

## Reopen when

The Lisp conformance and performance suites show that common batch routines cannot reach the entity
access target out of process even with batching ([OQ-10](../product/open-questions.md)).
