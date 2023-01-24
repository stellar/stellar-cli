use crate::util::{AssertExt, Sandbox, SorobanCommand};

// const SECRET_KEY: &str = "SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP";

// TODO figure out why stdin is failing
// #[test]
// fn set_identity() {
//     let dir = temp_dir();

//     let binding = Sandbox::new_cmd("config")
//         .current_dir(&dir)
//         .arg("identity")
//         .arg("add")
//         .arg("test_id")
//         .arg("--secret-key")
//         .write_stdin(SECRET_KEY)
//         .assert()
//         .success();
//     let output = binding.get_output();
//     println!("{}", String::from_utf8(output.stderr.clone()).unwrap());
// }

#[test]
fn read_identity() {
    let a = Sandbox::new_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert();
    let line = a.output_line();
    assert_eq!(line, "test_id")
}
