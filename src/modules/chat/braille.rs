mod symbols {
    pub(super) const BLANK: u16 = 0x2800;
    pub(super) const DOTS: [[u16; 2]; 4] = [
        [0x0001, 0x0008],
        [0x0002, 0x0010],
        [0x0004, 0x0020],
        [0x0040, 0x0080],
    ];
}

#[derive(Debug, Clone)]
pub struct BrailleProgress {
    width: usize,
    height: usize,
    length: usize,
    current: usize,
    label: Option<String>,
}

impl BrailleProgress {
    pub fn new(width: usize, height: usize, length: usize, label: Option<String>) -> Self {
        Self {
            width,
            height,
            length,
            current: 0,
            label,
        }
    }

    pub fn advance_progress(&mut self) {
        self.current = (self.current + 1) % self.pixel_length();
    }

    pub fn current_string(&self) -> String {
        self.string_for_progress(self.current)
    }

    pub fn string_for_progress(&self, current: usize) -> String {
        let mut chars: Vec<u16> = vec![symbols::BLANK; (self.width * self.height) as usize];

        let pixel_length = self.pixel_length();

        let mod_current = (current % pixel_length) as i32;
        let range = mod_current..(mod_current + self.length as i32);
        let out_of_range = mod_current + self.length as i32 - 1 - pixel_length as i32;

        let mut counter: i32 = 0;

        // Top
        for idx in 0..self.width {
            for i in 0..2 {
                if range.contains(&counter) || counter <= out_of_range {
                    let ch = chars.get_mut(idx).unwrap();
                    *ch |= symbols::DOTS[0][i];
                }
                counter += 1;
            }
        }
        counter -= 1;

        // Right
        for idx in (0..self.height).map(move |y| (y * self.width + self.width - 1)) {
            for i in 0..4 {
                if range.contains(&counter) || counter <= out_of_range {
                    let ch = chars.get_mut(idx).unwrap();
                    *ch |= symbols::DOTS[i][1];
                }
                counter += 1;
            }
        }
        counter -= 1;

        // Bottom
        for idx in (0..self.width)
            .rev()
            .map(move |x| (self.height - 1) * self.width + x)
        {
            for i in (0..2).rev() {
                if range.contains(&counter) || counter <= out_of_range {
                    let ch = chars.get_mut(idx).unwrap();
                    *ch |= symbols::DOTS[3][i];
                }
                counter += 1;
            }
        }
        counter -= 1;

        // Left
        for idx in (0..self.height).rev().map(move |y| y * self.width) {
            for i in (0..4).rev() {
                if range.contains(&counter) || counter <= out_of_range {
                    let ch = chars.get_mut(idx).unwrap();
                    *ch |= symbols::DOTS[i][0];
                }
                counter += 1;
            }
        }

        debug_assert_eq!(counter - 1, pixel_length as i32);

        // Add '\n' for each line.
        let mut result: String = "".to_owned();
        for (i, ch) in chars.iter().enumerate() {
            if i % self.width == 0 {
                result.push('\n');
            }
            result.push(std::char::from_u32(*ch as u32).unwrap());
        }

        // Add custom label string.
        if let Some(label) = &self.label {
            result.push(' ');
            result.push_str(&label);
        }

        result
    }

    fn pixel_length(&self) -> usize {
        let pixel_width = self.width * 2;
        let pixel_height = self.height * 4;
        let pixel_length = (pixel_width + pixel_height - 2) * 2;

        pixel_length
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use super::BrailleProgress;

    #[test]
    fn test_update() {
        let progress = BrailleProgress::new(10, 1, 3, None);
        for i in 0..1000 {
            println!("{}", progress.string_for_progress(i));
            sleep(std::time::Duration::from_millis(150));
        }
    }
}
