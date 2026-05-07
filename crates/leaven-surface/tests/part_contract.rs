use leaven_surface::{Part, PartView};

#[derive(Debug, Eq, PartialEq)]
enum SemanticView<'a> {
    SkillDocument { id: &'a str, body: &'a str },
    RawText(&'a str),
}

#[test]
fn part_semantics_live_in_the_surface_view() {
    let part = Part {
        id: "skill:format",
        address: "docs/skills/format.md",
        view: SemanticView::SkillDocument {
            id: "format",
            body: "formatting contract",
        },
    };

    assert_eq!(part.id, "skill:format");
    assert_eq!(part.address, "docs/skills/format.md");

    let SemanticView::SkillDocument { id, body } = part.view else {
        panic!("expected skill document view");
    };

    assert_eq!(id, "format");
    assert_eq!(body, "formatting contract");
}

#[test]
fn part_view_wraps_the_surface_payload() {
    let view = PartView {
        inner: SemanticView::RawText("raw payload"),
    };

    assert_eq!(view.inner, SemanticView::RawText("raw payload"));
}
