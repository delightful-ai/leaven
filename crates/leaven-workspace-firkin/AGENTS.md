`leaven-workspace-firkin` owns the Leaven workspace adapter for Firkin product
pods.

Keep Git artifact identity, optimizer policy, and frontier/admission logic out
of this crate. This crate only knows workspace allocation, command/file routing,
placement context, cleanup, and Firkin capability/refusal behavior.

The optional `firkin-facade` feature wires the real Firkin facade types from
`/Users/darin/vendor/github.com/apple/containerization`. Keep it out of default
features.

The optional `firkin-apple-vz-live` feature adds the Firkin single-node
Apple/VZ driver for ignored signed live tests only. Use
`scripts/run-signed-live-firkin-git-workspace-test.sh` with
`LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE` when proving the real product-pod path. The
default contract tests use fake runtimes so topology and Leaven workspace laws
stay testable without booting Apple/VZ.
