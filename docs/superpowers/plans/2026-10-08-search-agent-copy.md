# Search and agent-readable copy implementation plan

## Goal

Update both language versions of the README and project website so rfig is clearly discoverable as a terminal autocomplete tool and independent Fig alternative, while keeping claims grounded in current behavior.

## Architecture

Documentation-only edits across the standalone `rfig` and `rfig-site` repositories. Keep English primary, mirror factual content in Chinese, preserve page structure and existing navigation/install interactions.

## Tech Stack

Markdown and static HTML. No runtime or dependency changes.

## Global Constraints

- Describe only current support: macOS/Linux; zsh, Bash 4.4+, Fish 3.6+.
- Explain that candidate coverage depends on available shell definitions, scripts, generators, and conditional sandboxed help analysis.
- Do not claim AI completion, universal command coverage, Windows, or affiliation with Fig.
- Preserve canonical/hreflang links, language switching, and install controls.
- Do not commit or push.

## Tasks

1. Update English and Chinese READMEs with concise product definitions, relevant search terminology, and accurate FAQs while retaining technical guidance.
2. Update English and Chinese website titles, descriptions, Open Graph metadata, visible summaries, and FAQs consistently.
3. Review both repositories' diffs, links/metadata, bilingual parity, and whitespace; report any remaining caveats.
