# Function-level ledger

One record for every function of pinned X_ITE 16.5.0 (`34cdf9f97`, `oracle/x_ite-pin/src`), with the Rust symbol that holds it when the rules find one. It is separate from the 346-entry compatibility ledger (`ledger/ledger.json`) and links to it by the entry's upstream file. It does not change that ledger.

| file | content |
| --- | --- |
| `FUNCTION_LEDGER.json` | every record, one function on each line, and the counts (`meta`) |
| `FUNCTION_LEDGER.csv` | the same records as a table |
| `FUNCTION_LEDGER.md` | the named functions by X_ITE file |
| `FUNCTION_COVERAGE_SUMMARY.md` | the exact counts |
| `UNMAPPED_FUNCTIONS.md` | applicable functions with no Rust symbol found |
| `PORT_COMPLETION_BACKLOG.md` | the X_ITE files with open functions, in the order of work |

All six files are generated. To make them again (acorn 8 is in the `node_modules` of an X_ITE checkout):

```
node ledger/functions/tools/extract_functions.mjs <node_modules>/acorn oracle/x_ite-pin/src > /tmp/functions.jsonl
python3 -I -B ledger/functions/tools/build_function_ledger.py /tmp/functions.jsonl
python3 -I -B ledger/functions/tools/build_function_ledger.py /tmp/functions.jsonl --check
```

The rules, and what a record does not say, are in the module documentation of `tools/build_function_ledger.py`. In short: `cited` is the only verified mapping, `name` and `class` are candidates, and `none` is not proof that a behavior is absent.

A mapping changes when Rust code changes: a Rust comment that names the X_ITE function (`Owner.name`) makes the mapping `cited`. Make the files again after such a change.
