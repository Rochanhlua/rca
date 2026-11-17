use std::process::Command;

#[test]
fn native_crypto_matches_known_vectors() {
    let dir = std::env::temp_dir().join("rca_native_crypto");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t_crypto.rca");
    std::fs::write(
        &src,
        "import crypto\nprint(crypto.sha256(\"abc\"))\nprint(crypto.sha512(\"abc\"))\nprint(crypto.hmac_sha256(\"key\", \"msg\"))\n",
    )
    .unwrap();
    let rcac = env!("CARGO_BIN_EXE_rcac");
    let b = Command::new(rcac).arg("build").arg(&src).output().unwrap();
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));

    // On Windows, clang writes `t_crypto` without `.exe`; copy so Command can run it.
    let mut exe = dir.join("t_crypto");
    if cfg!(windows) {
        let with_ext = dir.join("t_crypto.exe");
        std::fs::copy(&exe, &with_ext).unwrap();
        exe = with_ext;
    }
    let out = Command::new(&exe).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    let expected = "\
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f
2d93cbc1be167bcb1637a4a23cbff01a7878f0c50ee833954ea5221bb1b8c628
";
    assert_eq!(got, expected);
}
