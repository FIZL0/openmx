//! P189: the build stamp. `build.rs` names the commit in the online build id (so two builds refuse each other's
//! lobbies by commit) and the main menu's corner; an unstamped dev build keeps handshake.rs's own id.

#[path = "../build.rs"]
#[allow(dead_code)]
mod stamp;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn a_stamped_builds_id_names_its_commit() {
    // the environment's stamp wins in any profile (tools/build.sh, tools/release.sh, pages.yml)
    for release in [false, true] {
        let c = stamp::commit(Some("abc1234"), release, Some("0123456789abcdef"), true);
        assert_eq!(c, "abc1234");
        assert_eq!(stamp::id(VERSION, &c), format!("{VERSION} abc1234, 30 Hz"));
    }
    // a release build without one asks git: HEAD's 7 characters, + when dirty
    assert_eq!(stamp::commit(None, true, Some("0123456789abcdef"), false), "0123456");
    assert_eq!(stamp::commit(Some(""), true, Some("0123456789abcdef"), true), "0123456+");
    assert_eq!(stamp::commit(None, true, None, false), "unknown");
    // two commits, two ids: neither build takes the other's lobby
    assert_ne!(stamp::id(VERSION, "0123456"), stamp::id(VERSION, "0123457"));
    // the corner's version is the tag's, without its v
    assert_eq!(stamp::version(Some("v0.2.0"), VERSION), "0.2.0");
    assert_eq!(stamp::version(Some("0.2.0"), VERSION), "0.2.0");
    assert_eq!(stamp::version(None, VERSION), VERSION);
}

#[test]
fn an_unstamped_dev_build_keeps_the_dev_id() {
    assert_eq!(stamp::commit(None, false, Some("0123456789abcdef"), true), "dev");
    assert_eq!(stamp::id(VERSION, "dev"), format!("{VERSION} dev, 30 Hz"));
    // and this build is one: the id the shell stamps, the corner's commit
    let commit = env!("OPENMX_COMMIT");
    assert_eq!(openmx_web::BUILD_ID, stamp::id(VERSION, commit));
    if commit == "dev" {
        // handshake.rs's own id when nobody stamps one: dev builds of one version meet each other
        assert_eq!(openmx_web::BUILD_ID, openmx_net::handshake::build());
        openmx_net::handshake::set_build(openmx_web::BUILD_ID);
        assert_eq!(openmx_net::handshake::build(), format!("{VERSION} dev, 30 Hz"));
    }
}
