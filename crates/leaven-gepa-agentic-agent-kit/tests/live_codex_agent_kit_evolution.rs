mod live_codex_agent_kit_evolution_support;
mod live_codex_agent_kit_fixture;

#[test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1; runs three live Codex stages"]
fn live_codex_agent_kit_evolves_through_stdio_checkpoint_and_next_codex_consumes_child() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex AgentKit e2e because LEAVEN_CODEX_LIVE != 1");
        return;
    }

    futures::executor::block_on(
        live_codex_agent_kit_evolution_support::run_live_codex_agentkit_evolution(),
    );
}
