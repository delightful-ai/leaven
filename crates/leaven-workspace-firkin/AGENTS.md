`leaven-workspace-firkin` owns the Leaven workspace adapter for Firkin product
pods.

Keep Git artifact identity, optimizer policy, and frontier/admission logic out
of this crate. This crate only knows workspace allocation, command/file routing,
placement context, cleanup, and Firkin capability/refusal behavior.

The optional `firkin-facade` feature is reserved for wiring the real Firkin
facade from `/Users/darin/vendor/github.com/apple/containerization`. Do not add
the path dependency until the live adapter uses it; a lockfile-only dependency
pulls the full Apple/VZ tree without proving runtime behavior. The default
contract tests use a fake runtime so topology and Leaven workspace laws stay
testable without booting Apple/VZ.
