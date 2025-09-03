#![no_std]
pub mod transport {
    use heapless::Vec;
    #[allow(unused_imports)]
    use micromath::F32Ext as _;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ZoomFactor {
        pub(crate) int: u8,
        pub(crate) fract: u8,
    }

    impl ZoomFactor {
        pub fn from_f32(float: f32) -> Self {
            Self {
                int: (float.floor() as u8).clamp(0x1, 0x1e),
                fract: ((float.fract() * 10.0) as u8).clamp(0x0, 0x9),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ZoomMode {
        ZoomIn,
        StopZoom,
        ZoomOut,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CenterPos {
        Default,
        Pos0,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GimbalMode {
        LockMode,
        FollowMode,
        FPVMode,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SiyiCommand {
        ControlAngle(i16, i16),
        AbsZoom(ZoomFactor),
        AutoZoom(ZoomMode),
        WorkingMode(GimbalMode),
        Center(CenterPos),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AckResult {
        Success,
        Error,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ControlAngles {
        pub yaw: i16,
        pub pitch: i16,
        pub roll: i16,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SiyiAck {
        Center(AckResult),
        ControlAngle(ControlAngles),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SiyiAckId {
        Center = 0x08,
        ControlAngle = 0x0E,
    }

    impl TryFrom<u8> for SiyiAckId {
        type Error = ();

        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0x08 => Ok(SiyiAckId::Center),
                0x0E => Ok(SiyiAckId::ControlAngle),
                _ => Err(()),
            }
        }
    }

    type PacketBuffer = Vec<u8, 255>;

    impl SiyiCommand {
        pub fn to_bytes(self) -> PacketBuffer {
            self.to_bytes_with_seq(0)
        }

        pub fn to_bytes_with_seq(self, seq: u8) -> PacketBuffer {
            match self {
                SiyiCommand::ControlAngle(yaw, pitch) => {
                    let mut byte_arr: PacketBuffer = PacketBuffer::new();
                    byte_arr.extend([0x55, 0x66, 0x01, 0x04, 0x00, seq, 0x00, 0x0e]);

                    byte_arr.extend(yaw.clamp(-1350, 1350).to_le_bytes());
                    byte_arr.extend(pitch.clamp(-900, 250).to_le_bytes());
                    byte_arr.extend(crc16_cal(&byte_arr).to_le_bytes());

                    byte_arr
                }
                SiyiCommand::AbsZoom(zoom) => {
                    let mut byte_arr: PacketBuffer = PacketBuffer::new();
                    byte_arr.extend([0x55, 0x66, 0x01, 0x02, 0x00, seq, 0x00, 0x0f]);

                    byte_arr.extend(zoom.int.to_be_bytes());
                    byte_arr.extend(zoom.fract.to_be_bytes());
                    byte_arr.extend(crc16_cal(&byte_arr).to_le_bytes());

                    byte_arr
                }
                SiyiCommand::AutoZoom(zoom) => {
                    let mut byte_arr: PacketBuffer = PacketBuffer::new();
                    byte_arr.extend([0x55, 0x66, 0x01, 0x01, 0x00, seq, 0x00, 0x05]);

                    let zoom: i8 = match zoom {
                        ZoomMode::ZoomIn => 1,
                        ZoomMode::StopZoom => 0,
                        ZoomMode::ZoomOut => -1,
                    };

                    byte_arr.extend(zoom.to_be_bytes());
                    byte_arr.extend(crc16_cal(&byte_arr).to_le_bytes());

                    byte_arr
                }
                SiyiCommand::WorkingMode(mode) => {
                    let mut byte_arr: PacketBuffer = PacketBuffer::new();
                    byte_arr.extend([0x55, 0x66, 0x01, 0x01, 0x00, seq, 0x00, 0x19]);

                    let mode: u8 = match mode {
                        GimbalMode::LockMode => 0,
                        GimbalMode::FollowMode => 1,
                        GimbalMode::FPVMode => 2,
                    };

                    byte_arr.extend(mode.to_be_bytes());
                    byte_arr.extend(crc16_cal(&byte_arr).to_le_bytes());

                    byte_arr
                }
                SiyiCommand::Center(center_pos) => {
                    let mut byte_arr: PacketBuffer = PacketBuffer::new();
                    byte_arr.extend([0x55, 0x66, 0x01, 0x01, 0x00, seq, 0x00, 0x08]);

                    let center_pos: u8 = match center_pos {
                        CenterPos::Default => 0,
                        CenterPos::Pos0 => 1,
                    };

                    byte_arr.extend(center_pos.to_be_bytes());
                    byte_arr.extend(crc16_cal(&byte_arr).to_le_bytes());

                    byte_arr
                }
            }
        }
    }

    impl SiyiAck {
        pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
            if bytes[0] != 0x55 || bytes[1] != 0x66 {
                return None;
            }

            if bytes.len() < 8
                || 10 + i16::from_le_bytes([bytes[3], bytes[4]]) as usize != bytes.len()
            {
                return None;
            }

            let crc = u16::from_le_bytes([bytes[bytes.len() - 2], bytes[bytes.len() - 1]]);
            if crc16_cal(&bytes[..bytes.len() - 2]) != crc {
                return None;
            }

            match SiyiAckId::try_from(bytes[7]).ok()? {
                SiyiAckId::Center => Some(SiyiAck::Center(if bytes[8] != 0 {
                    AckResult::Success
                } else {
                    AckResult::Error
                })),
                SiyiAckId::ControlAngle => Some(SiyiAck::ControlAngle(ControlAngles {
                    pitch: i16::from_be_bytes([bytes[8], bytes[9]]),
                    yaw: i16::from_be_bytes([bytes[10], bytes[11]]),
                    roll: i16::from_be_bytes([bytes[12], bytes[13]]),
                })),
            }
        }
    }

    const CRC16_TAB: [u16; 256] = [
        0x0, 0x1021, 0x2042, 0x3063, 0x4084, 0x50a5, 0x60c6, 0x70e7, 0x8108, 0x9129, 0xa14a,
        0xb16b, 0xc18c, 0xd1ad, 0xe1ce, 0xf1ef, 0x1231, 0x210, 0x3273, 0x2252, 0x52b5, 0x4294,
        0x72f7, 0x62d6, 0x9339, 0x8318, 0xb37b, 0xa35a, 0xd3bd, 0xc39c, 0xf3ff, 0xe3de, 0x2462,
        0x3443, 0x420, 0x1401, 0x64e6, 0x74c7, 0x44a4, 0x5485, 0xa56a, 0xb54b, 0x8528, 0x9509,
        0xe5ee, 0xf5cf, 0xc5ac, 0xd58d, 0x3653, 0x2672, 0x1611, 0x630, 0x76d7, 0x66f6, 0x5695,
        0x46b4, 0xb75b, 0xa77a, 0x9719, 0x8738, 0xf7df, 0xe7fe, 0xd79d, 0xc7bc, 0x48c4, 0x58e5,
        0x6886, 0x78a7, 0x840, 0x1861, 0x2802, 0x3823, 0xc9cc, 0xd9ed, 0xe98e, 0xf9af, 0x8948,
        0x9969, 0xa90a, 0xb92b, 0x5af5, 0x4ad4, 0x7ab7, 0x6a96, 0x1a71, 0xa50, 0x3a33, 0x2a12,
        0xdbfd, 0xcbdc, 0xfbbf, 0xeb9e, 0x9b79, 0x8b58, 0xbb3b, 0xab1a, 0x6ca6, 0x7c87, 0x4ce4,
        0x5cc5, 0x2c22, 0x3c03, 0xc60, 0x1c41, 0xedae, 0xfd8f, 0xcdec, 0xddcd, 0xad2a, 0xbd0b,
        0x8d68, 0x9d49, 0x7e97, 0x6eb6, 0x5ed5, 0x4ef4, 0x3e13, 0x2e32, 0x1e51, 0xe70, 0xff9f,
        0xefbe, 0xdfdd, 0xcffc, 0xbf1b, 0xaf3a, 0x9f59, 0x8f78, 0x9188, 0x81a9, 0xb1ca, 0xa1eb,
        0xd10c, 0xc12d, 0xf14e, 0xe16f, 0x1080, 0xa1, 0x30c2, 0x20e3, 0x5004, 0x4025, 0x7046,
        0x6067, 0x83b9, 0x9398, 0xa3fb, 0xb3da, 0xc33d, 0xd31c, 0xe37f, 0xf35e, 0x2b1, 0x1290,
        0x22f3, 0x32d2, 0x4235, 0x5214, 0x6277, 0x7256, 0xb5ea, 0xa5cb, 0x95a8, 0x8589, 0xf56e,
        0xe54f, 0xd52c, 0xc50d, 0x34e2, 0x24c3, 0x14a0, 0x481, 0x7466, 0x6447, 0x5424, 0x4405,
        0xa7db, 0xb7fa, 0x8799, 0x97b8, 0xe75f, 0xf77e, 0xc71d, 0xd73c, 0x26d3, 0x36f2, 0x691,
        0x16b0, 0x6657, 0x7676, 0x4615, 0x5634, 0xd94c, 0xc96d, 0xf90e, 0xe92f, 0x99c8, 0x89e9,
        0xb98a, 0xa9ab, 0x5844, 0x4865, 0x7806, 0x6827, 0x18c0, 0x8e1, 0x3882, 0x28a3, 0xcb7d,
        0xdb5c, 0xeb3f, 0xfb1e, 0x8bf9, 0x9bd8, 0xabbb, 0xbb9a, 0x4a75, 0x5a54, 0x6a37, 0x7a16,
        0xaf1, 0x1ad0, 0x2ab3, 0x3a92, 0xfd2e, 0xed0f, 0xdd6c, 0xcd4d, 0xbdaa, 0xad8b, 0x9de8,
        0x8dc9, 0x7c26, 0x6c07, 0x5c64, 0x4c45, 0x3ca2, 0x2c83, 0x1ce0, 0xcc1, 0xef1f, 0xff3e,
        0xcf5d, 0xdf7c, 0xaf9b, 0xbfba, 0x8fd9, 0x9ff8, 0x6e17, 0x7e36, 0x4e55, 0x5e74, 0x2e93,
        0x3eb2, 0xed1, 0x1ef0,
    ];

    fn crc16_cal(ptr: &[u8]) -> u16 {
        let mut crc = 0;
        for &byte in ptr {
            let temp = ((crc >> 8) & 0xff) as u8;
            let index = (byte ^ temp) as usize;
            let oldcrc16 = CRC16_TAB[index];
            crc = (crc << 8) ^ oldcrc16;
        }

        crc
    }
}

#[cfg(test)]
mod test {
    use crate::transport::{SiyiCommand, ZoomFactor};

    #[test]
    fn test_control_angle() {
        assert_eq!(
            SiyiCommand::ControlAngle(0, -90).to_bytes(),
            [
                0x55, 0x66, 0x01, 0x04, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0xa6, 0xff, 0xc0, 0x6e
            ]
        );
    }

    #[test]
    fn test_zoom() {
        assert_eq!(
            SiyiCommand::AbsZoom(ZoomFactor::from_f32(4.5)).to_bytes_with_seq(16),
            [
                0x55, 0x66, 0x01, 0x02, 0x00, 0x10, 0x00, 0x0f, 0x04, 0x05, 0x6b, 0x15
            ]
        );
    }
}
