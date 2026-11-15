<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# TablePro SQL tokenizer made UTF-8 safe (2026-11-15)

Source: issue #9. The tokenizer lives in `src/bin/tablepro/sql.rs`. Its consumers are `highlight_sql` and `segment_sql` in `src/bin/tablepro/tabs.rs`, `split_statements`, and the `Words` iterator.

## Decision

`tokenize` used to classify and advance one byte at a time with `b[i] as char`. A multi-byte scalar such as `é` (`C3 A9`) was read as two Latin-1 characters, and the identifier loop could stop between them. The next `&src[start..i]` then sliced inside a scalar and panicked, so `SELECT é` crashed the query editor.

The scanner now keeps one invariant. The cursor `i` is always a char boundary, and every branch advances by the scalar's `len_utf8()`. `scalar_at` reads through `str::get`, which rejects a non-boundary offset, so no helper can slice inside a scalar. ASCII delimiter lookahead (`--`, `/*`, `*/`, `'`, `"`, operator characters) still inspects bytes through `str::starts_with` and `str::find`, which is safe because an ASCII byte never appears inside a multi-byte scalar. The token API and the byte-offset ranges that `CodeEditor` consumes are unchanged.

The partition holds for every input. Tokens are non-empty, contiguous, cover the whole string, and start and end on char boundaries. `issue9_tokens_partition_utf8` checks this over a Unicode corpus, and `issue9_every_scalar_prefix_makes_progress` checks it for every char-boundary prefix of one mixed string.

## Unicode identifier policy

An identifier starts with an alphabetic scalar or `_` and continues with alphanumeric scalars or `_`, by the `char` predicates. Whitespace uses `char::is_whitespace`, so a no-break space and an em space are whitespace. Digits stay ASCII only. Any other scalar becomes one punctuation token. That covers emoji, symbols, and a combining mark such as the `\u{301}` in `e\u{301}`, which is not alphanumeric on its own, so `e\u{301}` lexes as the identifier `e` and a punctuation token.

This is a lexical policy that keeps the tokenizer total. It is not SQL Unicode identifier support, normalization, or grapheme-aware editing. Keyword and literal recognition stays ASCII-case-insensitive on the whole identifier slice, so `SELECTé` is one identifier, not the keyword `SELECT` followed by `é`.

## Doubled quotes

A doubled quote inside a string or quoted identifier is an escape and stays in one token. Before, the first closing quote ended the token, so `'it''s'` became two adjacent String tokens (`'it'`, `'s'`) and `"a""é"` two adjacent Ident tokens. Now each is one token. The highlighter therefore emits one span for the escaped literal, and `split_statements` treats the whole escaped literal as one token. An unterminated string, quoted identifier, or block comment extends to the end of input. `issue9_doubled_quotes_stay_one_token` and `issue9_unterminated_input_extends_to_eof` cover both.

## The Showcase highlighter stays unmigrated

`highlight` in `src/bin/showcase/pages/query_workbench.rs` is a separate ASCII-only scanner that does not use the TablePro tokenizer. It is left as is, for these reasons.

- It is panic-free for any valid string. A reviewer ran it over 38,416 multibyte strings without a panic. It tests and advances only on ASCII bytes (`is_ascii_digit`, `is_ascii_alphabetic`, `is_ascii_alphanumeric`), and a multi-byte scalar falls into the `i += 1` branch, one continuation or lead byte at a time. Every slice it takes starts and ends next to an ASCII byte or at a delimiter it found by `find`, and none lands inside a scalar.
- The #9 spec, section 5, keeps SQL scanning app-local. Sharing the TablePro tokenizer with the Showcase page would add a cross-app dependency that the issue does not ask for.
- Its seed text `FIRST_QUERY` is ASCII, so the rendered Showcase page does not depend on non-ASCII behavior. Migrating it would touch a path that approved baselines render, for no gain.

The one visible difference is cosmetic. The Showcase word loop stops at the first non-ASCII byte, so in `selecté` it highlights `select` as a keyword. TablePro reads `selecté` as one identifier and highlights nothing. This shows only for non-ASCII input on the Showcase page. The Showcase word-end rule was not aligned, because the change would alter Showcase behavior for a purely cosmetic gain.

## Evidence

- Regression tests in `src/bin/tablepro/sql.rs` and `src/bin/tablepro/tabs.rs`, all named `issue9_*`, plus the functional case `issue9_tablepro_query_editor_unicode_paste_keeps_text_and_editor_alive` in `tests/functional/tablepro.rs`.
- The policy is recorded for users in the Known limits list of `docs/current-state.md`.
- Out of scope: SQL grammar expansion, Unicode normalization, grapheme-aware cursor movement, and completion context. The completion-before-first-statement panic is a separate defect, tracked as #84 and recorded below.

## Completion context before the first statement (#84)

`model::context` found the current statement with `sql::statement_at`, which falls forward to the first statement when the cursor is before it. With `" SELECT id FROM users"` and the cursor at 0 it sliced `1..0` and panicked. A cursor inside a UTF-8 scalar also panicked on the first slice.

`completion_cursor` clamps the requested offset to the text and floors it to a char boundary. `context` and `complete` both use that cursor. `context` now tokenizes `src[..word_start]` and starts the clause scan after the last real `;` token. A `;` inside a string or comment is not a delimiter, and nothing after the cursor takes part. A cursor in leading whitespace gets `Clause::Start`, and so does a cursor after a completed statement.

### `statement_at` deleted

`statement_at` had two callers, both in `context` and `complete`, and both are gone. The function is deleted, not kept as dead public API, because its fallbacks (the previous statement, then the first one) are exactly what the completion fix must not use. Statement navigation and execution use `split_statements`, which is unchanged.

### Gap semantics for `complete`

`complete` ranks columns and aliases from the tables in one statement. That statement is now the untrimmed segment between the last real `;` that ends at or before the cursor and the next real `;` that starts at or after it, found by `statement_around` over `tokenize`.

Untrimmed matters. Matching the trimmed `split_statements` ranges finds no statement for a cursor in the trailing whitespace of `SELECT u.id FROM customers u WHERE ` (offset 35, past the trimmed end at 34). Completion then fell back to every catalog column, and offered `events.id` and `orders.id` instead of the `customers` columns and the alias `u`. Main did not have this defect, because `statement_at` fell back to the preceding statement. `completion_prefix_trailing_whitespace_keeps_the_statement_sources` covers `WHERE ` and `ORDER BY ` and failed before the fix. `completion_prefix_cursor_before_next_semicolon_keeps_its_statement` covers a cursor between a statement and its closing `;`.

The rules, by cursor position.

- Inside a statement or in its trailing whitespace, before the next `;`: that statement's tables and aliases.
- Right before a `;`: the statement that the `;` closes.
- Right after a `;`: the next segment, so the previous statement's aliases never apply (`completion_prefix_gap_gets_no_neighbour_aliases`).
- In whitespace between a `;` and the next statement's text: the segment includes that following statement, because no `;` separates them. The clause there is `Clause::Start`, which offers only start keywords and does not use the table list.
