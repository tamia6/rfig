//! Normalize terminal-specific Alt encodings into crossterm key events.
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::{io, time::Duration};

pub fn read() -> io::Result<Event> {
    let first = event::read()?;
    if cfg!(windows) {
        return Ok(first);
    }
    normalize(first, || {
        if event::poll(Duration::from_millis(35))? {
            event::read().map(Some)
        } else {
            Ok(None)
        }
    })
}

fn normalize(
    first: Event,
    mut next: impl FnMut() -> io::Result<Option<Event>>,
) -> io::Result<Event> {
    let Event::Key(escape) = first else {
        return Ok(first);
    };
    if escape.code != KeyCode::Esc || escape.kind == event::KeyEventKind::Release {
        return Ok(first);
    }
    let Some(second) = next()? else {
        return Ok(first);
    };
    let Event::Key(mut key) = second else {
        return Ok(second);
    };
    // Crossterm 0.28 emits Esc for the two leading ESC bytes, then emits the
    // remaining CSI bytes as characters. Other backends already return an arrow.
    if matches!(key.code, KeyCode::Char('[' | 'O')) {
        let mut sequence = String::new();
        for _ in 0..20 {
            let Some(Event::Key(part)) = next()? else {
                break;
            };
            let KeyCode::Char(ch) = part.code else {
                return Ok(Event::Key(part));
            };
            sequence.push(ch);
            if ch.is_ascii_alphabetic() || ch == '~' {
                break;
            }
        }
        if let Some(code) = sequence.chars().last().and_then(|last| match last {
            'A' => Some(KeyCode::Up),
            'B' => Some(KeyCode::Down),
            'C' => Some(KeyCode::Right),
            'D' => Some(KeyCode::Left),
            _ => None,
        }) {
            return Ok(Event::Key(KeyEvent::new(code, KeyModifiers::ALT)));
        }
        // No suffix is also a legitimate Alt+[ / Alt+O key.
        if !sequence.is_empty() {
            return Ok(first);
        }
    }
    key.modifiers |= KeyModifiers::ALT;
    Ok(Event::Key(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traditional_alt_arrows_match_structured_alt_events() {
        for (suffix, code) in [
            ('A', KeyCode::Up),
            ('B', KeyCode::Down),
            ('C', KeyCode::Right),
            ('D', KeyCode::Left),
        ] {
            let mut tail = vec![
                Event::Key(KeyCode::Char('[').into()),
                Event::Key(KeyCode::Char(suffix).into()),
            ]
            .into_iter();
            let actual = normalize(Event::Key(KeyCode::Esc.into()), || Ok(tail.next())).unwrap();
            assert_eq!(actual, Event::Key(KeyEvent::new(code, KeyModifiers::ALT)));
        }
    }
    #[test]
    fn single_escape_and_alt_numbers_remain_distinct() {
        let escape = Event::Key(KeyCode::Esc.into());
        assert_eq!(normalize(escape.clone(), || Ok(None)).unwrap(), escape);
        let mut next = Some(Event::Key(KeyCode::Char('3').into()));
        assert_eq!(
            normalize(escape, || Ok(next.take())).unwrap(),
            Event::Key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT))
        );
    }
}
