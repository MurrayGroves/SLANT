Manetsim is a high performance network simulation library targeted at MANETs.
You provide the behaviours and the library handles all the complicated orchestration and state management.

# Behaviours and packets

Each node has two behaviours - a movement behaviour and a node behaviour which are called each tick by the simulation.

Movement behaviours can access the current position of the node (along with whatever state you choose to store in the
behaviour itself), and each tick return a new position the node should be at.
Your simulation will be defined over a coordinate system of your choosing, so you can choose your desired floating point
type and dimensionality.
Some basic movement behaviours are provided in `builtin::move_behaviours` to help you get started.

Node behaviours receive a list of all packets the node received in a previous frame and may do whatever processing on
them is desired (including modifying the node's state), and can transmit new packets out into the network.

You'll likely define your node behaviours to work with a specific packet type so that your packets can store whatever
information is needed for your behaviours.
Packets can carry whatever data you like, they just need to define a couple methods from the `Packet` trait to allow the
simulation to optimise their delivery.

# Propagation Models

Propagation models are responsible for deciding which nodes receive transmitted packets.
A propagation model is defined over your coordinate system and given a transmitter and a receiver's position and
parameters must return a boolean indicating whether a packet is received or not.
If your requirements for accuracy are basic, there are some provided propagation models in
`builtin::propagation_models`.

# Examples

An example simulation is available in the `examples` directory, in which we define a flood routing node behaviour and
simulate it with a configurable number of nodes.
