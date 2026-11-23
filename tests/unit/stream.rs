use super::*;

#[test]
fn frames_are_read_as_written() {
    let mut a = df!("x" => [1, 2, 3]).unwrap();
    let mut b = df!("y" => ["p"], "z" => [true]).unwrap();
    let mut bytes = Vec::new();
    write(&mut bytes, "a", &mut a).unwrap();
    write(&mut bytes, "b é", &mut b).unwrap();

    let frames = read(&bytes).unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].0, "a");
    assert!(
        frames[0]
            .1
            .equals(&a)
    );
    assert_eq!(frames[1].0, "b é");
    assert!(
        frames[1]
            .1
            .equals(&b)
    );

    assert!(
        read(&[])
            .unwrap()
            .is_empty()
    );
    assert!(read(&bytes[..bytes.len() - 1]).is_err());
}
