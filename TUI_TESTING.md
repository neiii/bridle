# Ratatui TUI Testing Best Practices

Research findings for bridle-x0p.

## 1. TestBackend for Rendering Tests

```rust
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

#[track_caller]
fn test_widget(widget: impl Widget, expected: &Buffer) {
    let backend = TestBackend::new(expected.area.width, expected.area.height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| f.render_widget(widget, f.area())).unwrap();
    terminal.backend().assert_buffer(expected);
}

#[test]
fn test_my_widget() {
    let widget = MyWidget::new();
    test_widget(widget, &Buffer::with_lines([
        "┌────────┐",
        "│ Hello  │",
        "└────────┘",
    ]));
}
```

## 2. Snapshot Testing with Insta

```bash
cargo add insta --dev
cargo install cargo-insta
```

```rust
use insta::assert_snapshot;

#[test]
fn test_widget_snapshot() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| widget.render(f.area(), f.buffer_mut())).unwrap();
    assert_snapshot!(terminal.backend());
}
```

Workflow: `cargo test` → `cargo insta review` → accept/reject changes.

## 3. Event Handling Tests

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_navigation() {
    let mut app = App::new();
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_index(), 1);
    
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(app.should_quit());
}
```

## 4. Recommended Test Structure

| Test Type | What to Test | Approach |
|-----------|--------------|----------|
| Unit | Widget rendering | `Buffer::with_lines()` assertions |
| Unit | State transitions | Direct method calls |
| Integration | Full flows | TestBackend + event sequences |
| Snapshot | Complex UI | Insta snapshots |

## 5. Priority Tests for Bridle

1. **StatusBar widget** - Simple, high-value, isolated
2. **HarnessTabs widget** - Tests selection state
3. **DetailPane widget** - Tests scroll behavior
4. **App state transitions** - Tab cycling, navigation

## 6. Example Test File Structure

```
tests/
├── tui_widgets.rs      # Unit tests for individual widgets
├── tui_integration.rs  # Full app flow tests
└── snapshots/          # Insta snapshot files (auto-generated)
```

## References

- [Ratatui Testing Guide](https://ratatui.rs/recipes/testing/)
- [Insta Crate](https://insta.rs/)
- [TestBackend Docs](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
