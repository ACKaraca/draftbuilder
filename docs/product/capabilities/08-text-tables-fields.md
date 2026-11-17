# Text, tables and fields

Text, tables and fields carry the written content of a drawing: notes, labels, schedules and values
computed from the drawing itself. They keep their formatting and source data through DWG and DXF
exchange and report any font that cannot be resolved.

## Domain objects

- **Single-line text** — one line of text with a style, height, rotation, justification point and alignment.
- **Paragraph text** — a bounded block of formatted text with columns, paragraphs, lists and inline formatting.
- **Text style** — a named font, height, width factor, oblique angle and orientation flags.
- **Field** — an expression inside text whose displayed value is computed from the drawing, the file, a sheet set or a formula.
- **Table** — rows and columns of cells holding text, numbers, formulas, fields or blocks, formatted by a table style.
- **Data link** — a connection from a table or cell range to a range in an external spreadsheet file.
- **Extraction definition** — a saved selection of drawings, object types and properties that produces a table or file.

## Single-line and paragraph text

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-001 | Single-line text (`TEXT`, `DTEXT`) | Creates one or more single-line text objects typed in place at a start point, with height, rotation, style and one of the standard justifications including fit and align between two points; Space types a space and Enter starts the next line. | T1 | 1 |
| DB-TEXT-002 | Control codes | Single-line text accepts the conventional percent-sign codes for degree, plus/minus and diameter symbols and for underline and overline toggles, and Unicode escapes for any character. | T1 | 1 |
| DB-TEXT-003 | Paragraph text (`MTEXT`) | Creates paragraph text in a rectangular boundary with an in-place editor; width wraps lines, height is automatic or fixed, and nine attachment points set justification. | T1 | 1 |
| DB-TEXT-004 | Character formatting | In the editor, selected characters take font, height, bold, italic, underline, overline, strikethrough, color, width factor, tracking and oblique angle; formatting is stored as inline codes that round-trip through DWG and DXF. | T1 | 1 |
| DB-TEXT-005 | Paragraph formatting | Paragraphs take alignment, first-line and hanging indents, tab stops, line spacing and paragraph spacing, set through rulers or values. | T1 | 1 |
| DB-TEXT-006 | Lists | Numbered, lettered and bulleted lists renumber automatically on edit and support nesting by indent level. | T2 | 1 |
| DB-TEXT-007 | Columns | Paragraph text flows into static columns of fixed count and height or dynamic columns created as text grows, with gutter width and manual column heights. | T2 | 1 |
| DB-TEXT-008 | Stacked text and symbols | Fractions and tolerances stack as horizontal, diagonal or tolerance stacks from typed separators, editable after creation; a symbol menu inserts common drafting symbols and non-breaking spaces. | T1 | 1 |
| DB-TEXT-009 | Background mask | Paragraph text optionally masks objects behind it with the drawing background or a chosen color and a border offset; the mask plots. | T1 | 1 |
| DB-TEXT-010 | Editor tools | The editor imports plain or rich text files, changes case, finds and replaces within the object, checks spelling as the user types, and offers a source view of inline codes. | T2 | 1 |
| DB-TEXT-011 | Unicode and right-to-left | Text stores Unicode, shapes complex scripts, lays out right-to-left runs and converts legacy code-page text from older drawings on read. | T1 | 1 |

## Text editing helpers

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-012 | Edit text (`TEXTEDIT`) | Opens the in-place editor for text, paragraph text, attribute definitions and dimension text; double-clicking text does the same. | T1 | 1 |
| DB-TEXT-013 | Scale and justify (`SCALETEXT`, `JUSTIFYTEXT`) | Rescales many text objects about their own insertion or justification points to a height or factor, and changes justification without moving the text on screen. | T2 | 1 |
| DB-TEXT-014 | Align text (`TEXTALIGN`) | Aligns and spaces selected text objects to a picked text object, along a direction or with even spacing. | T2 | 1 |
| DB-TEXT-015 | Convert to paragraph text (`TXT2MTXT`) | Combines selected single-line texts into one paragraph text in reading order, keeping style and approximate spacing. | T2 | 1 |
| DB-TEXT-016 | Text to curves | Converts text into polylines or regions following the glyph outlines, for engraving, cutting and font-independent exchange. | T3 | 2 |
| DB-TEXT-017 | Arc-aligned text | Places text along an arc, with side, direction, spacing and offset; the text follows when the arc is edited. | T3 | 2 |
| DB-TEXT-018 | Enclose and number | Encloses selected text in a circle, slot or rectangle with an offset, and numbers selected text with a prefix, start value, increment and ordering rule. | T3 | 2 |

## Text styles and fonts

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-019 | Text styles (`STYLE`, `-STYLE`) | Defines named styles with a font, fixed or variable height, width factor, oblique angle, backwards, upside-down and vertical flags and an annotation-scaling flag (see DB-PLOT); editing a style updates every text that uses it. | T1 | 1 |
| DB-TEXT-020 | Outline and shape fonts | Renders TrueType and OpenType fonts and SHX shape fonts, including big fonts for East Asian text, with identical output on all three platforms. | T1 | 1 |
| DB-TEXT-021 | Font substitution and report | When a font is missing, a documented mapping file or a fallback rule substitutes another font; every substitution is listed in a per-drawing report and the original font name is preserved on save. | T1 | 1 |

## Fields

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-022 | Insert field (`FIELD`) | Inserts fields into text, paragraph text, attributes, table cells and dimension text; field kinds include date and time, file name and path, drawing properties, object properties, named objects, sheet set and sheet properties, hyperlinks, block attribute values and formulas. | T1 | 2 |
| DB-TEXT-023 | Object property fields | A field bound to an object property (for example a closed polyline's area) displays it with a chosen format and unit conversion and is a dependency in the regeneration graph, updating when the object changes. | T2 | 2 |
| DB-TEXT-024 | Field evaluation | Fields re-evaluate on open, save, plot, regeneration or on demand (`UPDATEFIELD`), according to a drawing setting; fields whose source is missing show a placeholder and are reported. | T1 | 2 |
| DB-TEXT-025 | Field display | Fields show a non-plotting background shade that can be turned off; converting a field to plain text freezes its current value. | T2 | 2 |

## Search and spelling

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-026 | Find and replace (`FIND`) | Searches text, paragraph text, attribute values, dimension text, table cells, field results and hyperlinks in the selection, current space or whole drawing, with case, whole-word and wildcard options; results list with zoom-to, and replace-all is one transaction. | T1 | 1 |
| DB-TEXT-027 | Spell check (`SPELL`) | Checks text in the chosen scope against a language dictionary and user dictionaries, suggesting replacements; dictionaries are plain files the user can edit and share. | T2 | 1 |

## Tables

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-028 | Table creation (`TABLE`) | Creates a table with title, header and data rows by row and column count or by window, empty, from a data link or from extracted data. | T1 | 1 |
| DB-TEXT-029 | Cell content and formats | Cells hold text, numbers, dates, currency, percentages, fields or block references, each with a data format, alignment, margins, rotation and borders. | T1 | 1 |
| DB-TEXT-030 | Cell formulas | Cells evaluate formulas with sum, average, count, cell references and arithmetic expressions; results update when referenced cells change. | T2 | 1 |
| DB-TEXT-031 | Table editing | Inserts, deletes, resizes and merges rows and columns, edits cells in place with keyboard navigation, and resizes with grips; a table can break into several linked parts across columns or sheets and flow upward or downward. | T1 | 1 |
| DB-TEXT-032 | Table styles (`TABLESTYLE`) | Named styles define per row type (title, header, data) the text style, height, color, fill, margins, borders and data format, and table direction; editing a style updates all tables using it. | T1 | 1 |
| DB-TEXT-033 | Table export (`TABLEEXPORT`) | Writes a table's displayed values to a CSV file. | T2 | 1 |

## Data links and extraction

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| DB-TEXT-034 | Spreadsheet data link (`DATALINK`) | Links a table or cell range to a sheet, named range or cell range in an XLSX or CSV file read and written by DraftBuilder itself, with no spreadsheet application installed; the path may be relative or absolute. | T2 | 2 |
| DB-TEXT-035 | Data link update (`DATALINKUPDATE`) | Detects source changes and marks linked tables; updating replaces values while keeping table formatting, and when write-back is allowed edited cells are written to the source. | T2 | 2 |
| DB-TEXT-036 | Data extraction (`DATAEXTRACTION`) | Extracts properties of chosen object types, block attribute values, dynamic block parameters and drawing information from the current drawing, a set of drawings or a folder, with filters, row combining, count and name columns, sorting and formatting. | T2 | 2 |
| DB-TEXT-037 | Extraction output and refresh | Writes extraction results to a table in the drawing, a CSV or XLSX file, or both; the definition is saved as a documented template file and re-running it refreshes the table and flags changed rows. | T2 | 2 |
| DB-TEXT-038 | Legacy attribute extraction | Writes attribute values to the legacy comma-delimited, space-delimited and DXF-subset extraction formats using a template, for older downstream tools. | T3 | 2 |
| DB-TEXT-039 | Headless extraction | Data extraction definitions run from the command-line application over many drawings and produce the same files without opening a window. | T3 | 2 |
