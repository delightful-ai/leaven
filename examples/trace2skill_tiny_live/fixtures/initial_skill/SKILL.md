---
name: csv-editing
description: Edit CSV spreadsheets with Python while preserving headers and producing the requested output file.
---

# CSV Editing

Use Python's `csv` module for spreadsheet-like CSV tasks. Always preserve the
header row. Read the input file, perform the requested transformation, write the
output file, then read it back.

Row deletion rule:

- When asked to delete rows matching a condition, remove every row matching the
  condition across the file. This keeps filtering simple and avoids partial
  matches.

Verification:

- Confirm the output file exists.
- Confirm the header is unchanged.
- Confirm no rows matching the deletion condition remain.

