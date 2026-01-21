# Change: Bulk import tags from Markdown

## Why
Adding tags one-by-one is slow and error-prone for users who maintain prompt snippets in Markdown. A bulk import flow reduces setup time while preserving explicit user confirmation.

## What Changes
- Add a "Bulk Add Tags" action in the tag manager UI to import tags from an uploaded Markdown file.
- Parse heading levels (# to ######) with @tag_name to build tag names and use the content between headings as tag content.
- Present a preview for confirmation, including duplicate tag handling with a second confirmation before updating existing tags.

## Impact
- Affected specs: tag-management (new)
- Affected code: frontend tag manager UI, new import dialog + parser logic, tag API usage for create/update
