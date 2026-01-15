mod helpers;

use crossterm::event::KeyCode;
use helpers::TestContext;

#[test]
fn core_workflow_creates_toggles_filters_deletes_undoes() {
    let mut ctx = TestContext::new();

    ctx.press(KeyCode::Enter);
    ctx.type_str("Smoke test entry");
    ctx.press(KeyCode::Enter);

    assert!(ctx.screen_contains("Smoke test entry"));

    ctx.press(KeyCode::Char('j'));
    ctx.press(KeyCode::Char('k'));

    ctx.press(KeyCode::Char(' '));
    assert!(ctx.screen_contains("[x]"));

    ctx.press(KeyCode::Char('d'));
    assert!(!ctx.screen_contains("Smoke test entry"));

    ctx.press(KeyCode::Char('u'));
    assert!(ctx.screen_contains("Smoke test entry"));

    let journal = ctx.read_journal();
    assert!(journal.contains("Smoke test entry"));

    ctx.verify_invariants();
}
