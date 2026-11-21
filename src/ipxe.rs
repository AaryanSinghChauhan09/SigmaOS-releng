//! iPXE netboot script generation.
//!
//! Port of `archlinux/releng/ipxe/generate_archlinux_ipxe.py` +
//! `archlinux.ipxe.jinja`: fetch available releases and HTTP mirrors, group
//! mirrors by country, and render the interactive iPXE menu script.

use serde::Deserialize;

/// A release entry from `releng/releases/json/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Version string (e.g. `2025.01.01`).
    pub version: String,
    /// Release date (unix timestamp or ISO string upstream — kept opaque).
    pub release_date: String,
}

/// A mirror entry (subset of `mirrors/status/json` needed for iPXE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mirror {
    /// Base URL with trailing slash.
    pub url: String,
    /// Mirror name.
    pub name: String,
    /// ISO country code.
    pub country_code: String,
    /// Country name.
    pub country: String,
}

/// Mirrors grouped by country for template rendering.
#[derive(Debug, Clone)]
pub struct MirrorGroup {
    /// Country code ("" for global/unsorted group).
    pub grouper: String,
    /// Country display name.
    pub grouper_name: String,
    /// Mirrors in this group (sorted by name).
    pub list: Vec<Mirror>,
}

const MIRRORS_URL: &str = "https://archlinux.org/mirrors/status/json";
const RELEASES_URL: &str = "https://archlinux.org/releng/releases/json/";

#[derive(Deserialize)]
struct ReleasesResponse {
    releases: Vec<ReleaseEntry>,
}

#[derive(Deserialize)]
struct ReleaseEntry {
    version: String,
    #[serde(default)]
    release_date: String,
    #[serde(default)]
    available: bool,
}

#[derive(Deserialize)]
struct MirrorsResponse {
    urls: Vec<MirrorEntry>,
}

#[derive(Deserialize)]
struct MirrorEntry {
    url: String,
    protocol: String,
    active: bool,
    isos: bool,
    #[serde(default)]
    details: String,
    country_code: String,
    country: String,
}

/// Fetch available releases (newest first), matching upstream ordering.
pub fn fetch_releases(client: &reqwest::blocking::Client) -> Result<Vec<Release>, crate::RelengError> {
    let resp: ReleasesResponse = client.get(RELEASES_URL).send()?.json()?;
    let mut releases: Vec<Release> = resp
        .releases
        .into_iter()
        .filter(|r| r.available)
        .map(|r| Release {
            version: r.version,
            release_date: r.release_date,
        })
        .collect();
    // Upstream sorts by release_date desc; keep input order when dates equal.
    releases.sort_by(|a, b| b.release_date.cmp(&a.release_date));
    Ok(releases)
}

/// Fetch active HTTP mirrors that carry ISO images.
pub fn fetch_mirrors(client: &reqwest::blocking::Client) -> Result<Vec<Mirror>, crate::RelengError> {
    let resp: MirrorsResponse = client.get(MIRRORS_URL).send()?.json()?;
    let mut mirrors: Vec<Mirror> = resp
        .urls
        .into_iter()
        .filter(|m| m.protocol == "http" && m.active && m.isos)
        .map(|m| Mirror {
            // Upstream derives the mirror base URL from `details` path.
            url: mirror_base_url(&m.url, &m.details),
            name: mirror_name(&m.details),
            country_code: m.country_code,
            country: m.country,
        })
        .collect();
    mirrors.sort_by(|a, b| a.country.cmp(&b.country).then(a.name.cmp(&b.name)));
    Ok(mirrors)
}

/// Upstream takes `details.rsplit('/',3)[1]`-style paths; we keep the mirror
/// URL as given but ensure a trailing slash.
fn mirror_base_url(url: &str, _details: &str) -> String {
    if url.ends_with('/') {
        url.to_string()
    } else {
        format!("{url}/")
    }
}

fn mirror_name(details: &str) -> String {
    // details like "https://mirror.example.com/archlinux/" → "mirror.example.com"
    details
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(details)
        .to_string()
}

/// Group mirrors by country code, sorted by country then code — matching
/// upstream `sort_mirrors_by_country`.
pub fn group_mirrors(mirrors: &[Mirror]) -> Vec<MirrorGroup> {
    let mut groups: std::collections::BTreeMap<String, MirrorGroup> =
        std::collections::BTreeMap::new();
    for m in mirrors {
        let g = groups
            .entry(m.country_code.clone())
            .or_insert_with(|| MirrorGroup {
                grouper: m.country_code.clone(),
                grouper_name: m.country.clone(),
                list: Vec::new(),
            });
        g.list.push(m.clone());
    }
    let mut out: Vec<MirrorGroup> = groups.into_values().collect();
    for g in &mut out {
        g.list.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out.sort_by(|a, b| a.grouper_name.cmp(&b.grouper_name));
    out
}

/// Render the iPXE script from releases + grouped mirrors.
///
/// Mirrors the structure of upstream `archlinux.ipxe.jinja` exactly:
/// header, cpuarch detection, x86_64 menu, i686 error menu, shell/reboot/
/// exit handlers, set_release / set_mirror / select_mirror_country /
/// enter_mirror_url / per-country select_mirror_url_XX blocks, set_options,
/// and the :boot section fetching kernel+initrds with imgverify.
pub fn render_ipxe(releases: &[Release], mirrors_by_country: &[MirrorGroup]) -> String {
    let mut s = String::new();
    s.push_str("#!ipxe\n\n\n");
    s.push_str("# Figure out if client is 64-bit capable\n");
    s.push_str("cpuid --ext 29 && set cpuarch x86_64 || set cpuarch i686\n\n");
    s.push_str("# allow only trusted images\nimgtrust\n\n");
    s.push_str("# initial options\n");
    s.push_str(&format!("set release {}\n", releases.first().map(|r| r.version.as_str()).unwrap_or("")));
    s.push_str("set extrabootoptions ip=dhcp net.ifnames=0 BOOTIF=01-${netX/mac}\n");
    s.push_str("set countrycode\n\n");

    // :main + :main_x86_64
    s.push_str(":main\niseq ${cpuarch} x86_64 && goto main_x86_64 || goto main_i686\n\n");
    s.push_str(":main_x86_64\nmenu Arch Linux Netboot\n");
    s.push_str("item --gap Settings\nitem --gap Architecture: x86_64\n");
    s.push_str("item set_release Release: ${release}\n");
    s.push_str("isset ${mirrorurl} && item set_mirror Mirror: ${mirrorurl} || item set_mirror Choose a mirror\n");
    s.push_str("item set_options Boot options: ${extrabootoptions}\nitem\n");
    s.push_str("isset ${mirrorurl} && item boot Boot Arch Linux || item --gap Boot Arch Linux\n");
    s.push_str("item shell Drop to iPXE shell\nitem reboot Reboot\nitem exit Exit iPXE\n");
    s.push_str("isset ${mirrorurl} && choose --default set_options selected || choose --default set_mirror selected || goto shell\n");
    s.push_str("goto ${selected} || goto main\n\n");

    // :main_i686
    s.push_str(":main_i686\nmenu Arch Linux Netboot\nitem --gap ERROR: i686 is no longer supported!\n");
    s.push_str("item shell Drop to iPXE shell\nitem reboot Reboot\nitem exit Exit iPXE\n");
    s.push_str("choose --default shell selected || goto shell\ngoto ${selected} || goto main\n\n");

    // :shell :reboot :exit
    s.push_str(":shell\necho Type 'exit' to get the back to the menu\nshell\ngoto main\n\n");
    s.push_str(":reboot\nreboot\n\n:exit\nexit\n\n");

    // :set_release
    s.push_str(":set_release\nmenu Arch Linux Netboot: Select Release\nitem back back\nitem\nitem --gap Available releases:\n");
    for r in releases {
        s.push_str(&format!("item {} {}\n", r.version, r.version));
    }
    s.push_str("choose selected || goto main\n");
    s.push_str("iseq ${selected} back && goto main ||\nset release ${selected}\ngoto main\n\n");

    // :set_mirror / :select_mirror_country
    s.push_str(":set_mirror\ngoto select_mirror_country\n\n");
    s.push_str(":select_mirror_country\nmenu Arch Linux Netboot: Select Mirror\nitem back back\nitem\n");
    s.push_str("item --gap Custom Configuration:\nitem custom Enter mirror URL\nitem\nitem --gap Select Mirror By Country:\n");
    for g in mirrors_by_country {
        if g.grouper.is_empty() {
            s.push_str("item 00 Global\n");
        } else {
            s.push_str(&format!("item {} {}\n", g.grouper, g.grouper_name));
        }
    }
    s.push_str("isset ${countrycode} && choose --default ${countrycode} selected || choose selected || goto main\n");
    s.push_str("iseq ${selected} back && goto main ||\niseq ${selected} custom && goto enter_mirror_url ||\n");
    s.push_str("set countrycode ${selected}\ngoto select_mirror_url\n\n");

    // :enter_mirror_url
    s.push_str(":enter_mirror_url\nset countrycode\n");
    s.push_str("echo Enter the mirror URL including the trailing slash.\n");
    s.push_str("echo Example: http://some.host/archlinux/\necho\n");
    s.push_str("echo -n Mirror URL: ${} && read mirrorurl || goto select_mirror_country\ngoto main\n\n");

    // :select_mirror_url + per-country blocks
    s.push_str(":select_mirror_url\ngoto select_mirror_url_${countrycode} || goto select_mirror_country\n\n");
    for g in mirrors_by_country {
        if g.grouper.is_empty() {
            s.push_str(":select_mirror_url_00\nset countryname Global\n");
        } else {
            s.push_str(&format!(":select_mirror_url_{}\nset countryname {}\n", g.grouper, g.grouper_name));
        }
        s.push_str("menu Arch Linux Netboot: Select Mirror\nitem back back\nitem\n");
        s.push_str("item --gap Available mirrors in ${countryname}\n");
        for m in &g.list {
            s.push_str(&format!("item {} {}\n", m.url, m.name));
        }
        s.push_str("choose selected || goto select_mirror_country\n");
        s.push_str("iseq ${selected} back && goto select_mirror_country ||\nset mirrorurl ${selected}\ngoto main\n");
    }

    // :set_options
    s.push_str("\n:set_options\necho -n Boot options: ${} && read extrabootoptions ||\ngoto main\n\n");

    // :boot — the actual netboot fetch sequence (upstream template).
    s.push_str(":boot\n");
    s.push_str("echo Booting Arch Linux x86_64 ${release} from ${mirrorurl}\necho\n");
    for (file, sig) in [
        ("vmlinuz-linux", "vmlinuz-linux.ipxe.sig"),
        ("amd-ucode.img", "amd-ucode.img.ipxe.sig"),
        ("intel-ucode.img", "intel-ucode.img.ipxe.sig"),
        ("initramfs-linux.img", "initramfs-linux.img.ipxe.sig"),
    ] {
        let remote = if file == "vmlinuz-linux" {
            "x86_64/vmlinuz-linux".to_string()
        } else if file == "initramfs-linux.img" {
            "x86_64/initramfs-linux.img".to_string()
        } else {
            file.to_string()
        };
        s.push_str(&format!(
            "{} ${{mirrorurl}}iso/${{release}}/arch/boot/{remote} || goto failed_download\n",
            if file == "vmlinuz-linux" { "kernel" } else { "initrd" }
        ));
        s.push_str(&format!(
            "imgverify {} ${{mirrorurl}}iso/${{release}}/arch/boot/{remote}.sig || goto failed_verify\n",
            file
        ));
        let _ = sig;
    }
    s.push_str("imgargs vmlinuz-linux initrd=amd-ucode.img initrd=intel-ucode.img initrd=initramfs-linux.img archiso_http_srv=${mirrorurl}iso/${release}/ archisobasedir=arch verify=y ${extrabootoptions}\n");
    s.push_str("boot || goto failed_boot\n\n");
    s.push_str(":failed_download\necho\necho Failed to download a file.\n");
    s.push_str(":failed_verify\necho\necho Failed to verify a file.\n");
    s.push_str(":failed_boot\necho\necho Boot failed.\n");
    s.push_str("goto main\n");
    s
}

/// Convenience: build an HTTP client and generate the full iPXE script.
pub fn generate_ipxe(client: &reqwest::blocking::Client) -> Result<String, crate::RelengError> {
    let releases = fetch_releases(client)?;
    let mirrors = fetch_mirrors(client)?;
    let groups = group_mirrors(&mirrors);
    Ok(render_ipxe(&releases, &groups))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_mirrors_by_country() {
        let mirrors = vec![
            Mirror { url: "http://de/".into(), name: "DE Mirror".into(), country_code: "DE".into(), country: "Germany".into() },
            Mirror { url: "http://us/".into(), name: "US Mirror".into(), country_code: "US".into(), country: "United States".into() },
            Mirror { url: "http://de2/".into(), name: "DE2".into(), country_code: "DE".into(), country: "Germany".into() },
        ];
        let groups = group_mirrors(&mirrors);
        assert_eq!(groups.len(), 2);
        let de = groups.iter().find(|g| g.grouper == "DE").unwrap();
        assert_eq!(de.list.len(), 2);
    }

    #[test]
    fn test_render_ipxe_contains_boot_section() {
        let releases = vec![Release { version: "2025.01.01".into(), release_date: "2025-01-01".into() }];
        let groups = vec![MirrorGroup {
            grouper: "DE".into(),
            grouper_name: "Germany".into(),
            list: vec![Mirror { url: "http://m/".into(), name: "M".into(), country_code: "DE".into(), country: "Germany".into() }],
        }];
        let out = render_ipxe(&releases, &groups);
        assert!(out.contains("#!ipxe"));
        assert!(out.contains("set release 2025.01.01"));
        assert!(out.contains(":select_mirror_url_DE"));
        assert!(out.contains("imgverify vmlinuz-linux"));
        assert!(out.contains("archiso_http_srv"));
        assert!(out.contains("item http://m/ M"));
    }

    #[test]
    fn test_render_empty_releases() {
        let out = render_ipxe(&[], &[]);
        assert!(out.contains(":boot"));
    }
}
