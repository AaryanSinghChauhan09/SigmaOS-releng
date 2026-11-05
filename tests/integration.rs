use sigmaos_releng::*;

#[test]
fn component_is_constructible_and_runnable() {
    // Arrange
    let c = sigmaos_releng::new("sigmaos-releng").expect("valid name");
    // Act
    let out = c.run().expect("run succeeds");
    // Assert
    assert!(out.contains("sigmaos-releng"));
}
