"""Tests for generated public-seam capability-call records."""

import msgspec
import pytest

from leaven._seam._wire.calls import (
    AgentRunCall,
    CapabilityCall,
    LmCompleteCall,
    LmContentText,
    OutputFiles,
    SandboxExecCall,
    WorkspaceMaterializeCall,
    WorkspaceReleaseCall,
)


def test_capability_call_decodes_locked_call_variants() -> None:
    """Example: Plan Call payloads preserve their concrete call identity."""

    calls = msgspec.json.decode(
        (
            b'[{"kind":"lm_complete","purpose":"python.test",'
            b'"messages":[{"role":"user","content":[{"kind":"text","text":"hi"}]}],'
            b'"output":{"kind":"final_message"},"input_classes":["public"]},'
            b'{"kind":"agent_run","runtime":"codex-cli",'
            b'"instructions":{"task":"fix it"},"output":{"kind":"final_message"},'
            b'"input_classes":["public"]},'
            b'{"kind":"sandbox_exec","workspace":"ws_1","argv":["echo","ok"],'
            b'"timeout_s":30,"output":{"kind":"files","paths":["out.txt"]},'
            b'"input_classes":["public"]},'
            b'{"kind":"workspace_materialize","candidate":"cand_1",'
            b'"mode":"copy_on_write","lifetime":"manual_release"},'
            b'{"kind":"workspace_release","workspace":"ws_1","force":true}]'
        ),
        type=list[CapabilityCall],
    )

    assert isinstance(calls[0], LmCompleteCall)
    lm_part = calls[0].messages[0].content[0]
    assert isinstance(lm_part, LmContentText)
    assert lm_part.text == "hi"
    assert isinstance(calls[1], AgentRunCall)
    assert calls[1].instructions.task == "fix it"
    assert isinstance(calls[2], SandboxExecCall)
    assert isinstance(calls[2].output, OutputFiles)
    assert calls[2].output.paths == ["out.txt"]
    assert isinstance(calls[3], WorkspaceMaterializeCall)
    assert calls[3].lifetime == "manual_release"
    assert isinstance(calls[4], WorkspaceReleaseCall)
    assert calls[4].force is True


def test_capability_call_rejects_unknown_kind() -> None:
    """Boundary check: Plan Call payloads are not arbitrary dictionaries."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            b'{"kind":"private_transport","payload":{}}',
            type=CapabilityCall,
        )


def test_lm_complete_rejects_unknown_content_part() -> None:
    """Boundary check: known LM content parts are tagged records."""

    with pytest.raises(msgspec.ValidationError):
        msgspec.json.decode(
            (
                b'{"kind":"lm_complete","purpose":"python.test",'
                b'"messages":[{"role":"user","content":[{"kind":"image","url":"x"}]}],'
                b'"output":{"kind":"final_message"},"input_classes":["public"]}'
            ),
            type=CapabilityCall,
        )


__all__ = []
