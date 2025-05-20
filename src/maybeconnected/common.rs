use midir::MidiIO;

pub trait MaybeConnected<IO: MidiIO> {
    fn unconnected(&self) -> Option<&IO>;
    fn connected_port_name(&self) -> Option<&str>;
    fn connect(self, port: IO::Port, portname: &str) -> Result<Self, (String, Self)>
    where
        Self: Sized;
    fn disconnect(self) -> Self;
}
