# rfig search and agent-readable copy design

## Goal

Improve discovery and factual extraction for rfig through clear, people-first copy in the English and Chinese READMEs and the English and Chinese project website. The English README and root website remain primary, consistent with `AGENTS.md`.

## Audience and intent

Developers looking for terminal autocomplete, command-line completion, a Fig alternative, or inline command-history suggestions. Search engines and answer-oriented AI systems should be able to identify what rfig is, which shells and operating systems it supports, how completion data is sourced, and where to install it without inferring unsupported capabilities.

## Content approach

Use direct product facts in visible headings, a short summary near the top, and concise FAQ answers. Place relevant terms in natural context rather than repeating them to manipulate rankings.

Core terms:

- terminal autocomplete
- command-line autocomplete / CLI autocomplete
- shell autocomplete
- IDE-style terminal autocomplete
- open-source Fig alternative

Supporting terms, where relevant:

- zsh, Bash, and Fish command completion
- dynamic subcommand completion
- inline, history-based command suggestions
- directory-scoped shell history
- zsh-autosuggestions alternative
- Rust terminal utility
- macOS and Linux terminal autocomplete

The Fig comparison must describe rfig as an independent project inspired by Fig and state that it is not affiliated with Fig. No claim should imply that all Fig features or all Fig specs are supported.

## Pages and changes

1. `README.md`: lead with a direct definition and a compact facts summary; improve discoverability of shell, operating-system, completion-source, history-suggestion, installation, and limitation answers; retain detailed technical installation and safety information.
2. `README.zh-CN.md`: mirror the same facts and structure in Chinese, with natural Chinese terminology and English search terms only where useful.
3. `rfig-site/index.html`: use a descriptive title, meta description, social-sharing metadata, and a concise visible product summary; make key features and support facts easy to scan; retain existing canonical, hreflang links, language switch, design, and installation controls.
4. `rfig-site/zh/index.html`: synchronize the Chinese title, description, sharing metadata, product summary, and FAQ facts with the English page.

## Source of truth and factual limits

The current README, `AGENTS.md`, and implementation are the source of product claims. State the shipped support as macOS/Linux with zsh, Bash 4.4+, and Fish 3.6+. Explain that candidate coverage and dynamic behavior depend on installed completion definitions and generators. Describe background help enrichment as sandboxed and conditional on platform support. History-based suggestions are local and directory-scoped.

Do not claim AI-powered completion, support for every command, a fixed number of supported CLIs, universal Fig-spec compatibility, support for Windows, or shell support beyond the listed shells. Do not turn testing gaps or caveats into guarantees.

## Metadata and answerability

- Make each page title identify rfig and the terminal-autocomplete category.
- Keep descriptions concise and aligned with visible page content.
- Add Open Graph title/description metadata that matches the page language and visible copy.
- Use explicit FAQ headings and self-contained answers for common factual questions. Do not add FAQ structured data or keyword-only pages.
- Preserve language-specific canonical URLs and existing English/Chinese alternates.

## Non-goals

- No code, feature, installation, shell-integration, or product behavior changes.
- No separate landing pages for individual keywords.
- No unsupported search-volume claims or guarantees about ranking or AI citations.
- No commit or push as part of this work.

## Review criteria

- Product definition and shell/OS support are clear in the first screen of each language version.
- English and Chinese pages agree on supported versions, installation, completion sources, history behavior, and limitations.
- Target terms appear naturally in titles, summaries, headings, and relevant FAQ answers.
- Claims match the current README and implementation; keyword stuffing and unsupported coverage claims are absent.
- Existing links, language switches, canonical/hreflang metadata, and installation controls remain intact.
