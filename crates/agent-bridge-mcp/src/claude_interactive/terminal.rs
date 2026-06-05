const MAX_PROBE_LEN: usize = 5;

const PROBES: &[(&[u8], &[u8])] = &[
    (b"\x1b[c", b"\x1b[?1;2c"),
    (b"\x1b[0c", b"\x1b[?1;2c"),
    (b"\x1b[>c", b"\x1b[>0;0;0c"),
    (b"\x1b[>0c", b"\x1b[>0;0;0c"),
    (b"\x1b[6n", b"\x1b[1;1R"),
    (b"\x1b[>q", b"\x1bP>|agent-bridge-claude\x1b\\"),
    (b"\x1b[18t", b"\x1b[8;40;120t"),
];

#[derive(Default)]
pub struct TerminalProbeHandler {
    pending: Vec<u8>,
}

pub struct TerminalProbeChunk {
    pub output: Vec<u8>,
    pub responses: Vec<Vec<u8>>,
}

impl TerminalProbeHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process(&mut self, bytes: &[u8]) -> TerminalProbeChunk {
        self.pending.extend_from_slice(bytes);
        let mut output = Vec::new();
        let mut responses = Vec::new();
        loop {
            if self.pending.is_empty() {
                break;
            }
            if let Some((probe, response)) = PROBES
                .iter()
                .find(|(probe, _)| self.pending.starts_with(probe))
            {
                self.pending.drain(..probe.len());
                responses.push(response.to_vec());
                continue;
            }
            if self.pending.len() < MAX_PROBE_LEN
                && PROBES
                    .iter()
                    .any(|(probe, _)| probe.starts_with(&self.pending))
            {
                break;
            }
            output.push(self.pending.remove(0));
        }
        TerminalProbeChunk { output, responses }
    }

    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_probe_handler_strips_probe_bytes_and_returns_responses() {
        let mut handler = TerminalProbeHandler::new();
        let chunk = handler.process(b"ready\x1b[c\x1b[>c\x1b[6n\x1b[>q\x1b[18tdone");
        assert_eq!(chunk.output, b"readydone");
        assert_eq!(
            chunk.responses,
            vec![
                b"\x1b[?1;2c".to_vec(),
                b"\x1b[>0;0;0c".to_vec(),
                b"\x1b[1;1R".to_vec(),
                b"\x1bP>|agent-bridge-claude\x1b\\".to_vec(),
                b"\x1b[8;40;120t".to_vec(),
            ]
        );
        assert!(handler.finish().is_empty());
    }

    #[test]
    fn terminal_probe_handler_handles_split_probe_bytes() {
        let mut handler = TerminalProbeHandler::new();
        let first = handler.process(b"\x1b[");
        assert!(first.output.is_empty());
        assert!(first.responses.is_empty());

        let second = handler.process(b"18tvisible");
        assert_eq!(second.output, b"visible");
        assert_eq!(second.responses, vec![b"\x1b[8;40;120t".to_vec()]);
        assert!(handler.finish().is_empty());
    }
}
