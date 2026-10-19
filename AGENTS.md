---
project_common_language: en
---

# pptx2img Development Guidelines

## Language policy

- English is the language of public project artifacts: documentation (including
  README and development guides), CLI help, error and warning messages, code
  comments, test names, and new issues, pull requests, and commit messages.
- Discussions with the user, design conversations, and conversational progress
  reports may remain in Japanese. Follow the user's preferred language.
- Apply this policy to new content and content being updated. Existing content
  may be translated incrementally; a repository-wide translation is not required.
- Do not rewrite historical issues or commits solely to change their language.
- Preserve Japanese text when it is part of an input, fixture, font name, or other
  data whose exact text matters to the behavior being tested or reported.
- This project-specific policy supersedes inherited Japanese-only requirements
  for project artifacts. Explicit user instructions take precedence.

## Independent implementation and rendering quality

- ibis-office is a source of inspiration. Do not copy, port, or translate its
  code, tests, or assets into this project.
- Implement independently using specifications such as OOXML and OpenType,
  public library APIs, tests created in this project, and rendering comparisons.
- Distinguish having consulted public ibis-office materials from not reusing its
  code. Do not claim that its source has never been viewed.
- Declare third-party libraries as dependencies and address their licenses and
  attribution. Independent implementation does not mean avoiding all libraries.
- LibreOffice is a development-time comparison reference, not a CLI runtime
  dependency.
- Do not relax thresholds to hide rendering differences. Preserve known
  differences together with materials that reproduce them.
- Record unsupported and approximate rendering as warnings. Do not imply that
  compatibility has been verified when it has not.
