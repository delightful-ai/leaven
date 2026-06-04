## Python SDK Test Layout

Keep unit, example, and law tests outside `src/` and mirror the source module
path they prove. For example, test `src/leaven/builders/case.py` in
`tests/builders/test_case.py`, and test `src/leaven/_seam/plans.py` in
`tests/_seam/test_plans.py`.

Use `tests/integration/` for multi-module process or subprocess scenarios.
Use `tests/support/` for shared fake clients, fixtures, and assertion helpers
once two or more test modules need the same machinery.

Do not add new grab-bag files for unrelated SDK claims. If a test file no
longer names one clear source module or one clear scenario, split it before
adding more cases.

`scripts/check_quality_contract.py` enforces the currently locked mirrored-test
map for public-seam wire, run readback, and case-builder modules. Extend that
map in the same change when a new module becomes part of the SDK proof surface.
