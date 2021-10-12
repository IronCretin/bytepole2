#![feature(stdio_locked)]

use std::fmt;
use std::io::{self, prelude::*, ErrorKind, StdinLock, StdoutLock};
use std::ops::{ControlFlow, ControlFlow::*};

#[derive(Debug, Clone)]
pub struct Machine<I, O> {
    mem: [u8; 0x100],
    pc: u8,
    stack: u8,
    stdin: I,
    stdout: O,
}

impl Default for Machine<StdinLock<'static>, StdoutLock<'static>> {
    fn default() -> Self {
        Self {
            mem: [0; 0x100],
            pc: 0,
            stack: 0xff,
            stdin: io::stdin_locked(),
            stdout: io::stdout_locked(),
        }
    }
}

impl<T: AsRef<[u8]>> From<T> for Machine<StdinLock<'static>, StdoutLock<'static>> {
    fn from(src: T) -> Self {
        let mut m = Self::default();
        let buf = src.as_ref();
        m.mem[..buf.len()].copy_from_slice(buf);
        m
    }
}

impl<I, O> Machine<I, O> {
    #[inline]
    pub fn push(&mut self, val: u8) {
        self.mem[self.stack as usize] = val;
        self.stack = self.stack.wrapping_sub(1);
    }
    #[inline]
    pub fn pop(&mut self) -> u8 {
        self.stack = self.stack.wrapping_add(1);
        self.mem[self.stack as usize]
    }

    pub fn dump(&self) -> impl fmt::Display + '_ {
        struct Dump<'a, I, O>(&'a Machine<I, O>);
        impl<I, O> fmt::Display for Dump<'_, I, O> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                writeln!(f, "   pc: {}", self.0.pc)?;
                writeln!(f, "stack: {}", self.0.stack)?;
                write!(f, "   ")?;
                for j in 0..16 {
                    write!(f, " _{:x}", j)?;
                }
                writeln!(f)?;
                for i in 0..16 {
                    write!(f, "{:x}_ ", i)?;
                    for j in 0..16 {
                        if 16 * i + j == self.0.pc as usize || 16 * i + j == self.0.stack as usize {
                            write!(f, " \u{1b}[7m{:02x}\u{1b}[m", self.0.mem[16 * i + j])?;
                        } else {
                            write!(f, " {:02x}", self.0.mem[16 * i + j])?;
                        }
                    }
                    write!(f, "  ")?;
                    for j in 0..16 {
                        let ch = self.0.mem[16 * i + j];
                        match ch {
                            // printable characters
                            0x20..=0x7e => write!(f, "{}", ch as char)?,
                            _ => write!(f, ".")?,
                        }
                    }
                    writeln!(f)?;
                }
                Ok(())
            }
        }
        Dump(self)
    }
}
impl<I, O> Machine<I, O>
where
    I: BufRead,
    O: Write,
{
    pub fn step(&mut self) -> io::Result<ControlFlow<()>> {
        let code = self.mem[self.pc as usize];
        match code {
            // halt
            b'x' => return Ok(Break(())),
            // constant numbers
            b'0'..=b'9' => self.push(code - b'0'),
            b'a'..=b'f' => self.push(code - b'a' + 10),
            b'.' => {
                let lo = self.pop();
                let hi = self.pop();
                self.push(hi << 4 | lo);
            }
            // input/output
            b'i' => {
                write!(self.stdout, "> ")?;
                self.stdout.flush()?;

                let mut buf = String::new();
                self.stdin.read_line(&mut buf)?;
                let n = buf
                    .trim()
                    .parse()
                    .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
                self.push(n);
            }
            b'o' => {
                let a = self.pop();
                writeln!(self.stdout, "{}", a)?
            }
            b':' => {
                let mut buf = [0];
                self.stdin.read_exact(&mut buf)?;
                self.push(buf[0])
            }
            b'\'' => {
                let a = self.pop();
                self.stdout.write_all(&[a])?;
            }
            b'"' => {
                let d = format!("{}", self.dump());
                writeln!(self.stdout, "{}", d)?;
            }
            // stack manipulation
            b'@' => {
                let b = self.pop();
                let a = self.pop();
                self.push(b);
                self.push(a);
            }
            b'(' => {
                let a = self.pop();
                self.push(a);
                self.push(a);
            }
            b')' => {
                self.pop();
            }
            // math
            b'+' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_add(b))
            }
            b'-' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_sub(b))
            }
            b'*' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_mul(b))
            }
            b'/' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_div(b))
            }
            b'%' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_rem(b))
            }
            b'^' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a.wrapping_pow(b.into()))
            }
            // logical and comparison
            b'!' => {
                let a = self.pop();
                self.push(if a == 0 { 1 } else { 0 });
            }
            b'=' => {
                let b = self.pop();
                let a = self.pop();
                self.push(if a == b { 1 } else { 0 })
            }
            b'<' => {
                let b = self.pop();
                let a = self.pop();
                self.push(if a < b { 1 } else { 0 })
            }
            b'>' => {
                let b = self.pop();
                let a = self.pop();
                self.push(if a > b { 1 } else { 0 })
            }
            // bitwise
            b'~' => {
                let a = self.pop();
                self.push(!a);
            }
            b'|' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a | b)
            }
            b'&' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a & b)
            }
            b'X' => {
                let b = self.pop();
                let a = self.pop();
                self.push(a ^ b)
            }
            // control flow
            b'g' => {
                let addr = self.pop();
                self.pc = addr;
                return Ok(Continue(()));
            }
            b'j' => {
                let addr = self.pop();
                let cond = self.pop();
                if cond != 0 {
                    self.pc = addr;
                    return Ok(Continue(()));
                }
            }
            // memory acess
            b'l' => {
                let addr = self.pop();
                let val = self.mem[addr as usize];
                self.push(val);
            }
            b's' => {
                let addr = self.pop();
                let val = self.pop();
                self.mem[addr as usize] = val;
            }
            _ => return Ok(Break(())),
        }
        self.pc = self.pc.wrapping_add(1);
        Ok(Continue(()))
    }
}

fn main() -> io::Result<()> {
    let mut buf = String::new();
    loop {
        buf.clear();
        print!(">>> ");
        io::stdout().flush()?;
        if io::stdin().read_line(&mut buf)? == 0 {
            break;
        }
        let mut m = Machine::from(&buf);
        // println!("{}", m.dump());
        loop {
            match m.step()? {
                Break(_) => break,
                _ => {}
            }
            // println!("{}", m.dump());
        }
        // break;
    }
    Ok(())
}
