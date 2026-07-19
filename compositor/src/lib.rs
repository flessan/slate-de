use smithay::{
    backend::input::KeyState,
    delegate_compositor, delegate_output, delegate_seat, delegate_shm,
    reexports::wayland_server::{Display, DisplayHandle},
    wayland::{
        compositor::{CompositorHandler, CompositorState},
        output::{OutputHandler, OutputState},
        seat::{Seat, SeatHandler, SeatState},
        shm::{ShmHandler, ShmState},
    },
};

pub struct SlateState {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub output_state: OutputState,
    pub seat_state: SeatState,
    pub shm_state: ShmState,
    pub seat: Seat<Self>,
}

impl SlateState {
    pub fn new(display: &mut Display<Self>) -> Self {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let output_state = OutputState::new();
        let mut seat_state = SeatState::new();
        let shm_state = ShmState::new::<Self>(&dh, Vec::new());

        let mut seat = seat_state.new_wl_seat(&dh, "slate-0");

        SlateState {
            display_handle: dh,
            compositor_state,
            output_state,
            seat_state,
            shm_state,
            seat,
        }
    }
}

// Smithay Delegates
delegate_compositor!(SlateState);
delegate_output!(SlateState);
delegate_seat!(SlateState);
delegate_shm!(SlateState);

impl CompositorHandler for SlateState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    fn client_compositor_state<'a>(
        &self,
        _client: &'a smithay::reexports::wayland_server::Client,
    ) -> &'a smithay::wayland::compositor::ClientCompositorState {
        unimplemented!()
    }
    fn commit(
        &mut self,
        _surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
    }
}

impl OutputHandler for SlateState {}

impl ShmHandler for SlateState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for SlateState {
    type KeyboardFocus = smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
    type PointerFocus = smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {}
    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::wayland::seat::CursorImageStatus,
    ) {
    }
}
