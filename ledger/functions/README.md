# Function-level ledger

One record for every function of pinned X_ITE 16.5.0 (`34cdf9f97`, `oracle/x_ite-pin/src`), with its disposition. It is separate from the 346-entry compatibility ledger (`ledger/ledger.json`) and links to it by the entry's upstream file. It does not change that ledger, and it is not a measure of how much of X_ITE the Rust engine implements.

| file | tracked | content |
| --- | --- | --- |
| `tools/extract_functions.mjs`, `tools/build_function_ledger.py` | yes | the generator |
| `dispositions.jsonl` | yes | the inspected dispositions, one JSON record on each line (the only source of a status) |
| `FUNCTION_COVERAGE_SUMMARY.md` | yes, generated | the exact counts |
| `PORT_COMPLETION_BACKLOG.md` | yes, generated | the X_ITE files with open functions, in the order of work |
| `FUNCTION_LEDGER.manifest.json` | yes, generated | bytes, records and sha256 of the four files below |
| `FUNCTION_LEDGER.json` | no | every record, one function on each line, and the counts (`meta`) |
| `FUNCTION_LEDGER.csv` | no | the same records as a table |
| `FUNCTION_LEDGER.md` | no | the named functions by X_ITE file |
| `UNMAPPED_FUNCTIONS.md` | no | the applicable functions that nobody has inspected |

The four large files are not in Git: GitHub gives no patch for files of that size, so no review can read them (owner decision 2026-11-11). The builder makes them from the pinned source, and the manifest is their identity. The maintainer keeps a copy in the dated report directory of the lane, outside the repository.

To make all files (acorn 8.18.0; CI installs that version):

```
node ledger/functions/tools/extract_functions.mjs <node_modules>/acorn oracle/x_ite-pin/src > /tmp/functions.jsonl
python3 -I -B ledger/functions/tools/build_function_ledger.py /tmp/functions.jsonl --large-dir <directory>
```

To check them, as the "Function ledger manifest" step of `rust-tests.yml` does:

```
python3 -I -B ledger/functions/tools/build_function_ledger.py /tmp/functions.jsonl --large-dir <directory> --check
python3 -I -B ledger/functions/tools/build_function_ledger.py --verify-dir <directory>
```

`--check` fails when a tracked generated file or the manifest would change. `--verify-dir` fails when one of the four files is missing or its bytes, records or sha256 differ from the manifest. It also verifies a stored copy.

## What a status says

The nine statuses and the format of a disposition are in the module documentation of `tools/build_function_ledger.py`. The rules:

- `UNRESOLVED_MAPPING` is the default. It means that nobody has inspected the function. It does not mean that the implementation is missing.
- Any other status comes only from a record of `dispositions.jsonl`, written by someone who read the X_ITE function and the Rust code.
- A source comment, a symbol name or a test name never sets a status. Do not add a comment to Rust code to change a count.
- `IMPLEMENTED_AND_VERIFIED` needs the Rust symbol, a named test, and `evidence`: what the test does and why that is the equivalent behavior. The builder checks that the symbol and the test exist. It cannot check the claim; the review of the pull request does.
- The manifest proves that the data is the generator's output. It does not prove that a mapping is correct.
- The output does not depend on the Rust sources. It changes when the pin, `dispositions.jsonl` or the `upstream` file names of `ledger/ledger.json` change; make the files again in that pull request.
- An implementation batch records dispositions for the functions that it inspected. Nobody classifies the whole list first.

`--hints <directory>` writes a scan of the Rust sources for candidates (`MAPPING_HINTS.csv`, `MAPPING_HINTS_SUMMARY.md`). A hint is where to look; it is not a status and it is not tracked.
