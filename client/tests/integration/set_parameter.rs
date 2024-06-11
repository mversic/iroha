use eyre::Result;
use iroha::{
    client::{self, QueryResult},
    data_model::prelude::*,
};
use test_network::*;

#[test]
fn can_change_parameter_value() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(11_135).start_with_runtime();
    wait_for_genesis_committed(&vec![test_client.clone()], 0);

    let parameter = "?BlockTime=4000".parse()?;
    let parameter_id = "BlockTime".parse()?;
    let param_box = SetParameter::new(parameter);

    let old_params = test_client
        .request(client::parameter::all())?
        .collect::<QueryResult<Vec<_>>>()?;
    let param_val_old = old_params
        .iter()
        .find(|param| param.id() == &parameter_id)
        .expect("Parameter should exist")
        .val();

    test_client.submit_blocking(param_box)?;

    let new_params = test_client
        .request(client::parameter::all())?
        .collect::<QueryResult<Vec<_>>>()?;
    let param_val_new = new_params
        .iter()
        .find(|param| param.id() == &parameter_id)
        .expect("Parameter should exist")
        .val();

    assert_ne!(param_val_old, param_val_new);
    Ok(())
}

#[test]
fn parameter_propagated() -> Result<()> {
    let (_rt, _peer, test_client) = <PeerBuilder>::new().with_port(10_985).start_with_runtime();
    wait_for_genesis_committed(&vec![test_client.clone()], 0);

    let too_long_domain_name: DomainId = "0".repeat(2_usize.pow(8)).parse()?;
    let create_domain = Register::domain(Domain::new(too_long_domain_name));
    let _ = test_client
        .submit_blocking(create_domain.clone())
        .expect_err("Should fail before ident length limits update");

    let parameter = "?WSVIdentLengthLimits=1,256_LL".parse()?;
    let param_box = SetParameter::new(parameter);
    test_client.submit_blocking(param_box)?;

    test_client
        .submit_blocking(create_domain)
        .expect("Should work after ident length limits update");
    Ok(())
}
