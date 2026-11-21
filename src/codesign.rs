//! Secure Boot code-signing key-pair workflow.
//!
//! Port of `codesigning/create_codesigning_key_pair.sh`: generates an
//! Authenticode signing certificate (self-signed, RSA-2048, ~10y) and its
//! detached `.cer`/`.pem` artifacts, then registers it for sbsigntool.
//! This module produces the plan/command sequence; actual crypto is
//! delegated to `sbsign`/`openssl`/`pesign` at run time.

/// Parameters for the code-signing key pair.
#[derive(Debug, Clone)]
pub struct KeyPairPlan {
    /// Subject CN for the certificate.
    pub common_name: String,
    /// Organization.
    pub organization: String,
    /// Validity in days.
    pub days: u32,
    /// Key algorithm.
    pub key_type: String,
    /// Output directory for artifacts.
    pub out_dir: String,
    /// Detached certificate output filename.
    pub cert_file: String,
    /// Private key output filename.
    pub key_file: String,
}

impl Default for KeyPairPlan {
    fn default() -> Self {
        Self {
            common_name: "Arch Linux Secure Boot Signing".into(),
            organization: "Arch Linux".into(),
            days: 3650,
            key_type: "rsa:2048".into(),
            out_dir: "signing".into(),
            cert_file: "archlinux-db.cer".into(),
            key_file: "archlinux-db.key".into(),
        }
    }
}

/// Certificate enrollment plan (for MOK/shim or UEFI db injection).
#[derive(Debug, Clone)]
pub struct CertPlan {
    /// Certificate file to enroll.
    pub cert: String,
    /// Target keyring/db description.
    pub description: String,
}

/// Build a key-pair plan and the shell command sequence upstream would run.
///
/// Returns (plan, commands) where commands are the openssl/sbsign
/// invocations in order. Commands are strings for the caller to execute —
/// this component intentionally does not spawn processes.
pub fn plan_key_pair(plan: &KeyPairPlan) -> (KeyPairPlan, Vec<String>) {
    let key = format!("{}/{}", plan.out_dir, plan.key_file);
    let cert = format!("{}/{}", plan.out_dir, plan.cert_file);
    let commands = vec![
        format!(
            "openssl req -new -x509 -newkey {} -keyout {} -out {} -days {} -nodes -subj '/CN={}/O={}'",
            plan.key_type, key, cert, plan.days, plan.common_name, plan.organization
        ),
        format!(
            "openssl x509 -in {} -outform DER -out {}.der",
            cert, cert
        ),
        format!(
            "sbsign --key {} --cert {} <efi-binary>",
            key, cert
        ),
    ];
    (plan.clone(), commands)
}

/// Plan certificate enrollment into a UEFI/MOK database.
pub fn plan_cert_enrollment(cert_path: &str) -> CertPlan {
    CertPlan {
        cert: cert_path.to_string(),
        description: "Arch Linux Release Engineering".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_plan() {
        let plan = KeyPairPlan::default();
        assert_eq!(plan.days, 3650);
        assert_eq!(plan.key_type, "rsa:2048");
    }

    #[test]
    fn test_plan_commands() {
        let (plan, cmds) = plan_key_pair(&KeyPairPlan::default());
        assert_eq!(cmds.len(), 3);
        assert!(cmds[0].contains("openssl req"));
        assert!(cmds[0].contains(&plan.out_dir));
        assert!(cmds[2].contains("sbsign"));
    }

    #[test]
    fn test_cert_enrollment_plan() {
        let c = plan_cert_enrollment("/keys/db.cer");
        assert_eq!(c.cert, "/keys/db.cer");
    }
}
