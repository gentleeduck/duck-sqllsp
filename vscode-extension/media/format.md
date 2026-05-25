# Format settings

duck-sqllsp ships a DataGrip-style aligner + optional shell-out to
`sql-formatter` (npm). Both are driven by `.duck-sqllsp.toml`.

```toml
[duck_sqllsp.style]
keyword    = "upper"     # SELECT / FROM / WHERE
function   = "lower"     # now() / coalesce()
type       = "upper"     # INT / TEXT
identifier = "preserve"  # table + column names untouched

[duck_sqllsp.style.createTable]
alignColumns        = true
openParenOnNewLine  = true
constraintsAtEnd    = true
columnGap           = 4
groupIndexes        = true

[duck_sqllsp.style.formatter]
language               = "postgresql"
tabWidth               = 2
keywordCase            = "upper"
linesBetweenQueries    = 1
dataTypeCase           = "upper"
functionCase           = "lower"
denseOperators         = false
newlineBeforeSemicolon = false
expressionWidth        = 80
logicalOperatorNewline = "before"
```

Diagnostic severity overrides go under the same root:

```toml
[duck_sqllsp.rules]
sql001 = "warning"    # downgrade unresolved-table to warn
sql169 = "off"        # skip owner-to-unknown-role
sql051 = "error"      # promote LIMIT-without-ORDER
```

Per-line suppression via inline comments:

```sql
SELECT * FROM users WHERE bogus = 1; -- duck-sqllsp: ignore sql002
INSERT INTO t (a) VALUES ('x', 'y'); -- duck-sqllsp: ignore
-- duck-sqllsp: ignore-next-line sql038
INSERT INTO t (a) VALUES ('x', 'y');
```
