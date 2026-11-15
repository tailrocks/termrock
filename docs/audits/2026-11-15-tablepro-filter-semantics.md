<!-- SPDX-License-Identifier: LicenseRef-Proprietary -->

# TablePro filter semantics (issue 10)

Every TablePro filter operator now evaluates its stated row semantics. Before this change, `Not contains` and `not in list` produced no predicate, `Regex` ran as a substring match, and the match-all or match-any choice was never sent to the simulator. Enabled filters are now one expression tree. Match-all and match-any are the connectives over the enabled filters, `Between` is one grouped child, and `Not` wraps the negated operators.

The simulator uses SQL three-valued logic. A comparison, text test, list test, or regex on NULL is unknown, and only `is NULL` and `is not NULL` are true on NULL. Negation keeps unknown rows out, so `not contains` does not admit NULL.

A filter that cannot evaluate stays in the editor with its error on the field at fault. The editor checks required values, list items, integer and number operands, regex syntax, and range order before it closes. The view behind it is not replaced.

Regex is Rust `regex` syntax, case-sensitive, compiled once per load. Text comparison is case-insensitive. Query-editor `LIKE` keeps its wildcard meaning and is now case-sensitive, while `ILIKE` ignores case, and both print back as the keyword that was typed. The UI contains, starts-with, and ends-with operators match their values literally. The SQL preview escapes quotes and LIKE wildcards, and quotes column names that are reserved words such as `user`.

Numbers compare by value, so `in list` agrees with `=` for the same operand (`IN ('25.9')` matches a numeric 25.90). Integer columns compare as `i64`, so values past 2^53 stay distinct. A boolean column accepts only `true` or `false` as an operand and rejects anything else in the editor. SQL text literals come from one helper, `model::quote_text`, shared with mutation previews.

Regression tests are the `issue10_` tests in `src/bin/tablepro/tabs.rs` and `src/bin/tablepro/sql.rs`.
