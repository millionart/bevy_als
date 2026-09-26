use bevy_als::extensions::inertialized_linked_layer_activation;

fn main() {
    let first = inertialized_linked_layer_activation(None, None, 0, 3, true, true);
    let switch = inertialized_linked_layer_activation(Some(0), Some(3), 1, 4, true, true);
    let return_to_previous =
        inertialized_linked_layer_activation(Some(1), Some(4), 0, 5, true, true);
    let relevant_again = inertialized_linked_layer_activation(Some(0), Some(5), 0, 8, true, true);
    let continuous = inertialized_linked_layer_activation(Some(0), Some(8), 0, 9, true, true);
    let passed = first.reset_child
        && !first.forward_request
        && switch.reset_child
        && switch.forward_request
        && return_to_previous.reset_child
        && return_to_previous.forward_request
        && relevant_again.reset_child
        && relevant_again.forward_request
        && !continuous.reset_child
        && !continuous.forward_request;
    let verdict = if passed { "PASS" } else { "FAIL" };
    println!(
        "[BF102.25][VERDICT] first=({},{}) switch=({},{}) return=({},{}) relevant_again=({},{}) continuous=({},{}) -> {verdict}",
        first.reset_child,
        first.forward_request,
        switch.reset_child,
        switch.forward_request,
        return_to_previous.reset_child,
        return_to_previous.forward_request,
        relevant_again.reset_child,
        relevant_again.forward_request,
        continuous.reset_child,
        continuous.forward_request,
    );
    if !passed {
        std::process::exit(25);
    }
}
