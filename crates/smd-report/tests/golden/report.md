# smd scan report

## Scan completeness

- files_total: 120
- partial: 1
- unparsed: 1
  - oversize: 1
- domains: event
- content_incomplete: (none)
- order certainty: inferred (source: directory_scan)
- activation certainty: fallback
- snapshot method: not_applicable

**scan incomplete: some files or sources were not fully read, so the findings below may be missing conflicts**

## Mods (4)

| key | name | role | activation | known by | position | version | path |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `vanilla` | Base game | vanilla | active | always active | 0 | - | `game` |
| `281990_1001` | Alpha Traits | workshop | unknown | not established | unknown | 1\.2 | `workshop/1001` |
| `281990_1002` | Beta \[Fix\]\(https\://e\.x\) | workshop | unknown | not established | unknown | 2\.0\.1 | `workshop/1002` |
| `local_gamma` | Gamma | local | unknown | not established | unknown | 1\.2 | `mod/gamma` |

- descriptor (`281990_1001`): supported\_version is not a valid pattern

## Findings (3)

### 1. `core.file_overwrite` `common/traits/00_traits.txt`

- certainty: completeness=unparsed activation=conditional order=fallback semantics=unmodelled
- gaps: order_unknown
- files: 2
- identical: false

Participants (2):

| role | key | mod | path | span |
| --- | --- | --- | --- | --- |
| winner | `281990_1001` | Alpha | `common/traits/00_traits.txt` | \@12\+40 line 3 |
| shadowed | `281990_1002` | Beta | `common/traits/00_traits.txt` | \@12\+40 line 3 |

### 2. `core.file_overwrite` `localisation/english/l_english.yml`

- certainty: completeness=partial activation=conditional order=fallback semantics=unmodelled
- gaps: order_unknown
- files: 2
- identical: false

Participants (3):

| role | key | mod | path | span |
| --- | --- | --- | --- | --- |
| winner | `281990_1001` | Alpha Traits | `localisation/english/l_english.yml` | \@12\+40 line 3 |
| shadowed | `281990_1002` | Beta \[Fix\]\(https\://e\.x\) | `localisation/english/l_english.yml` | \@12\+40 line 3 |
| identical | `local_gamma` | Gamma | `localisation/english/l_english.yml` | \@12\+40 line 3 |

### 3. `core.file_overwrite` `events/pirate_events.txt`

- certainty: completeness=unparsed activation=conditional order=fallback semantics=unmodelled
- gaps: order_unknown
- files: 2
- identical: false

Participants (1):

| role | key | mod | path | span |
| --- | --- | --- | --- | --- |
| contributor | `local_gamma` | Gamma | `events/pirate_events.txt` | \@12\+40 line 3 |

## Diagnostics (1)

- warn `clausewitz.unbalanced_brace` `events/pirate_events.txt`: brace never closed
