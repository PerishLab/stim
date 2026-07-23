use stim::santi::boundary;

#[test]
fn boundaries() {
    assert_eq!(boundary("event: wake\ndata: {}\n\n"), Some((20, 2)));
    assert_eq!(boundary("event: wake\r\ndata: {}\r\n\r\n"), Some((21, 4)));
    assert_eq!(boundary("event: wake\ndata: {}"), None);
}
