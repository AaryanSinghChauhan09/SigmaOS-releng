//! Integration tests for sigmaos-releng.

use releng::{Mirror, MirrorGroup, Release, group_mirrors, plan_cert_enrollment, plan_key_pair, render_ipxe, KeyPairPlan};

fn mirror(url: &str, name: &str, cc: &str, country: &str) -> Mirror {
    Mirror { url: url.into(), name: name.into(), country_code: cc.into(), country: country.into() }
}

#[test]
fn test_ipxe_full_render() {
    let releases = vec![
        Release { version: "2025.06.01".into(), release_date: "2025-06-01".into() },
        Release { version: "2025.05.01".into(), release_date: "2025-05-01".into() },
    ];
    let groups = group_mirrors(&[
        mirror("http://mirror.de/archlinux/", "mirror.de", "DE", "Germany"),
        mirror("http://mirror.us/archlinux/", "mirror.us", "US", "United States"),
    ]);
    let out = render_ipxe(&releases, &groups);
    // Structure checks mirroring upstream template.
    assert!(out.starts_with("#!ipxe"));
    assert!(out.contains("cpuid --ext 29"));
    assert!(out.contains("item 2025.06.01 2025.06.01"));
    assert!(out.contains("item 2025.05.01 2025.05.01"));
    assert!(out.contains(":select_mirror_url_DE"));
    assert!(out.contains(":select_mirror_url_US"));
    assert!(out.contains("kernel ${mirrorurl}iso/${release}/arch/boot/x86_64/vmlinuz-linux"));
    assert!(out.contains("imgverify initramfs-linux.img"));
    assert!(out.contains("archisobasedir=arch verify=y"));
}

#[test]
fn test_mirror_grouping_sorted() {
    let groups = group_mirrors(&[
        mirror("http://z/", "z", "ZZ", "Zed"),
        mirror("http://a/", "a", "AA", "Alpha"),
    ]);
    // Sorted by country name: Alpha before Zed.
    assert_eq!(groups[0].grouper_name, "Alpha");
}

#[test]
fn test_codesign_plan_commands() {
    let plan = KeyPairPlan { common_name: "Test CA".into(), ..Default::default() };
    let (_, cmds) = plan_key_pair(&plan);
    assert!(cmds.iter().any(|c| c.contains("Test CA")));
}

#[test]
fn test_cert_enrollment() {
    let c = plan_cert_enrollment("out.cer");
    assert!(!c.description.is_empty());
}
