#[path = "../mixins/astral_cruiser/onboarding.rs"]
mod astral_onboarding;

use astral_onboarding::{Event, Progress, Step, STEP_COUNT};

#[test]
fn starts_at_call_ship() {
    let p = Progress::default();
    assert_eq!(p.current(), Some(Step::CallShip));
    assert!(!p.all_done());
    assert!(!p.hidden);
    assert_eq!(p.finished_at, None);
    assert_eq!(p.done, [false; STEP_COUNT]);

    assert_eq!(Step::CallShip.title(), "Вызвать корабль");
    assert_eq!(Step::CallShip.key_hint(), "K");
}

#[test]
fn out_of_order_marks() {
    let mut p = Progress::default();
    // Fire event 4 (Wheel) before earlier steps
    p.observe(Event::OpenedWheel);
    assert!(p.done[Step::Wheel.index()]);
    assert_eq!(p.done.iter().filter(|&&b| b).count(), 1);
    // current step is still Step::CallShip (first not done)
    assert_eq!(p.current(), Some(Step::CallShip));

    // Fire event 7 (Struck)
    p.observe(Event::Struck);
    assert!(p.done[Step::Strike.index()]);
    assert_eq!(p.done.iter().filter(|&&b| b).count(), 2);
    assert_eq!(p.current(), Some(Step::CallShip));
}

#[test]
fn current_skips_done() {
    let mut p = Progress::default();
    p.observe(Event::CalledShip);
    assert_eq!(p.current(), Some(Step::Board));

    p.observe(Event::Boarded);
    assert_eq!(p.current(), Some(Step::Seat));

    p.observe(Event::Seated);
    assert_eq!(p.current(), Some(Step::Fly));

    // Jump ahead and do Wheel and Fire
    p.observe(Event::OpenedWheel);
    p.observe(Event::Fired);
    // Still needs Fly
    assert_eq!(p.current(), Some(Step::Fly));

    // Complete Fly
    p.observe(Event::Flew);
    // Now skips Wheel and Fire, landing on Orbit
    assert_eq!(p.current(), Some(Step::Orbit));
}

#[test]
fn roundtrip_serialization() {
    // Example from spec/prompt: "111000000h"
    let mut p = Progress::default();
    p.observe(Event::CalledShip);
    p.observe(Event::Boarded);
    p.observe(Event::Seated);
    p.hidden = true;
    assert_eq!(p.to_string(), "111000000h");
    assert_eq!(Progress::parse(&p.to_string()), p);

    // With finished_at timestamp
    p.finished_at = Some(9876.5);
    let s = p.to_string();
    assert_eq!(Progress::parse(&s), p);

    // All done without hidden
    let mut all = Progress::default();
    for step in Step::ALL {
        all.done[step.index()] = true;
    }
    assert_eq!(all.to_string(), "111111111");
    assert_eq!(Progress::parse(&all.to_string()), all);

    // Reset restores clean default
    p.reset();
    assert_eq!(p, Progress::default());
}

#[test]
fn garbage_parses_default() {
    assert_eq!(Progress::parse(""), Progress::default());
    assert_eq!(Progress::parse("10101"), Progress::default());
    assert_eq!(Progress::parse("corrupt string here"), Progress::default());
    assert_eq!(Progress::parse("111000002"), Progress::default());
    assert_eq!(Progress::parse("xyz123456789"), Progress::default());
    assert_eq!(Progress::parse("111000000h:not_a_float"), Progress {
        done: [true, true, true, false, false, false, false, false, false],
        hidden: true,
        finished_at: None,
    });
}

#[test]
fn all_done_when_nine() {
    let mut p = Progress::default();
    let events = [
        Event::CalledShip,
        Event::Boarded,
        Event::Seated,
        Event::Flew,
        Event::OpenedWheel,
        Event::Fired,
        Event::Orbit,
        Event::Struck,
        Event::OpenedTablet,
    ];

    for (i, &ev) in events.iter().enumerate() {
        assert!(!p.all_done());
        p.observe(ev);
        if i + 1 < events.len() {
            assert!(!p.all_done());
        }
    }

    assert!(p.all_done());
    assert_eq!(p.current(), None);
    assert_eq!(p.done, [true; STEP_COUNT]);
}
