use crate::hardware::device::Device;
use crate::hardware::interfaces::DeviceOps;
use crate::hardware::bus::*;
use crate::hardware::cpu::*;
use crate::hardware::ram::*;

use std::cell::RefCell;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;
use std::ops::Deref;
use std::rc::Rc;
use std::{io, io::Error};
use crossterm::style::Print;
use tui::Frame;
use tui::backend::Backend;
use tui::{backend::CrosstermBackend, Terminal};
use tui::{
    style::*,
    text::*,
    widgets::*,
    layout::*,
};
use crossterm::{
    event::*,
    execute,
};

use super::disassembler::Disassembler;

pub struct State<'a> {
    pub bus: Rc<RefCell<Bus<'a>>>,
    pub cpu: Rc<RefCell<Cpu<'a>>>,
    pub dis: Disassembler
}

pub struct App<'a> {
    pub memory_page_index: i32,
    pub previous_machine_state: Vec<State<'a>>,
    pub inner_machine_state: Rc<RefCell<State<'a>>>,
}

impl<'a> State<'a> {
    pub fn build_view<B: Backend>(f: &mut Frame<B>, app: &App)  {
        let size = Rect::new(0, 0, f.size().width, f.size().height);
        let block = Block::default().style(Style::default().bg(Color::White).fg(Color::Black));
        f.render_widget(block, size);

        State::memory_viewer(f, app);
        State::program_viewer(f, app);
        State::processor_viewer(f, app);
    }

    pub fn initiate_state() -> Rc<RefCell<State<'a>>> {
        let ram = Rc::new(RefCell::new(Device::Ram(Ram::new())));
        let bus = Rc::new(RefCell::new(Bus::new()));
        let cpu = Rc::new(RefCell::new(Cpu::new()));
        
        bus.borrow_mut().add_device(ram.clone());

        (*cpu).borrow_mut().bus = Some(bus.clone());


        let state = Rc::new(RefCell::new(State {
            bus : Rc::clone(&bus),
            cpu : Rc::clone(&cpu),
            dis : Disassembler::new()
        }));

        

        (*bus).borrow_mut().write(0xFFFC, 0x00);
        (*bus).borrow_mut().write(0xFFFC + 1, 0x80);
        (*cpu).borrow_mut().reset();


        state
    }

    pub fn load_program_from_file(possible_path : Option<String>) -> Result<Vec<u8>, Error> {
        let prompt = "Enter a file name: ";
        
        let parse_file = |path: String| {
            if let Ok(metadata_file) = File::options()
                                                    .read(true)
                                                    .open(path.trim())
            {
                // readlines 
                let reader = BufReader::new(metadata_file);
                let mut lines = Vec::new();
                let mut bytes = Vec::new();
                for line in reader.lines() {
                    // read line
                    if let Ok(line) = line {
                        lines.push(line);
                    }
                }
                
                for line in lines {
                    let mut split = line.split_whitespace();
                    while let Some(byte) = split.next() {
                        if let Ok(byte) = u8::from_str_radix(byte, 16) {
                            bytes.push(byte);
                        }
                    }
                }
                return Ok(bytes);
            }
            return Err(Error::new(ErrorKind::Other, "Error"))
        };

        if let Some(path) = possible_path {
            return parse_file(path)
        } 

        if let Ok(_) = execute!(io::stdout(), Print(prompt)) {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            return parse_file(input);
        }
        return Err(Error::new(ErrorKind::Other, "Error"))

    }

    pub fn memory_viewer<B: Backend>(f: &mut Frame<B>, app: &App)  {
        let size = Rect::new(0, 0, (f.size().width as f32 * 0.70) as u16, f.size().height);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(5)
            .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
            .split(size);

        let block = Block::default().style(Style::default().bg(Color::White).fg(Color::Black));
        f.render_widget(block, size);

        let build_page_table = |bus: &Rc<RefCell<Bus>>, page: u16| {
            let header_cells = (0..=16)
                .map(|i| format!("{:02X}", i))
                .map(|s| Cell::from(s).style(Style::default().fg(Color::Red)));
            let header_row = Row::new(header_cells)
                .style(Style::default().bg(Color::Blue))
                .height(1)
                .bottom_margin(1);
            
            let mut rows = vec![];
            for i in 0..=16 {
                let mut row_data = vec![
                    Cell::from(format!("{:02X}", i + 1)).style(Style::default().fg(Color::Black).bg(Color::Blue)),
                ];
                for j in 0..16 {
                    let address = (page << 8) + (i * 16 + j);
                    let value = bus.borrow_mut().read(address);
                    let cell = Cell::from(format!("{:04X}", value)).style(Style::default().fg(Color::Black));
                    row_data.push(cell);
                }
                let row = Row::new(row_data).height(2);
                rows.push(row);
            }


            let page_title = format!("Page {:02X}", page);
            let table = Table::new(rows)
                .header(header_row)
                .block(Block::default().borders(Borders::ALL).title(page_title))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ")
                .widths(&[
                    Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5),
                    Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5),
                    Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5),
                    Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5),
                    Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5), Constraint::Percentage(5),
                ]);
            table
        };

        
        let upper_bound = if app.memory_page_index < 16 {
            16
        } else if app.memory_page_index > 256 - 16 {
            255
        } else {
            app.memory_page_index + 8
        };

        let lower_bound = if app.memory_page_index < 16 {
            0
        } else if app.memory_page_index > 256 - 16 {
            255 - 16
        } else {
            app.memory_page_index - 7
        };
        
        let mut rows_vec = Vec::new();
        rows_vec.push(Row::new([Cell::from("[ Pages ]")]));
        (lower_bound..upper_bound)
            .map(|i| format!("[{}{:02X}]", if i == app.memory_page_index { ">>" } else { "" } ,i))
            .map(|s| Span::styled(s, Style::default().fg(Color::LightBlue)))
            .map(|s| Row::new([s]).height(2))
            .for_each(|r| rows_vec.push(r));
        // put element at the beggining of the vector
        let page_selection_table = Table::new(rows_vec)
            .block(Block::default().borders(Borders::ALL).title("Pages"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ")
            .widths(&[Constraint::Percentage(100)]);
        f.render_widget(page_selection_table, chunks[0]);
        
        let local_app_state_deref = (*app.inner_machine_state).borrow_mut();
        let table = build_page_table(&local_app_state_deref.bus, app.memory_page_index as u16);
        f.render_widget(table, chunks[1]);

    }

    pub fn processor_viewer<B: Backend>(f: &mut Frame<B>, app: &App)  {
        let size = Rect::new((f.size().width as f32 * 0.70) as u16, 0, (f.size().width as f32 * 0.30) as u16, (f.size().height as f32 * 0.60) as u16);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(5)
            .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
            .split(size);

        let block = Block::default().style(Style::default().bg(Color::White).fg(Color::Black));
        f.render_widget(block, size);

        
        let build_registers_list = |cpu: &Rc<RefCell<Cpu>>| {
            let cpu_local = cpu.borrow_mut();
            let list_elements = vec![
                ListItem::new(Spans::from(vec![Span::raw(format!(" A: {:02X}", cpu_local.registers.a))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!(" X: {:02X}", cpu_local.registers.x))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!(" Y: {:02X}", cpu_local.registers.y))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("PC: {:04X}", cpu_local.registers.pc))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("SP: {:02X}", cpu_local.registers.sp))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!(" P: {:02X}", cpu_local.registers.status))])), 
            ];
            let list = List::new(list_elements)
                .block(Block::default().borders(Borders::ALL).title("Registers"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");
            list
        };

        let build_status_view = |cpu: &Rc<RefCell<Cpu>>| {
            let cpu_local = cpu.borrow_mut();
            let list_elements = vec![
                ListItem::new(Spans::from(vec![Span::raw(format!("N: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::N)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("V: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::O)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("B: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::B)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("D: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::D)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("I: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::I)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("Z: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::Z)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("C: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::C)))])), 
                ListItem::new(Spans::from(vec![Span::raw(format!("U: {}", cpu_local.registers.get_flag(crate::hardware::registers::Flag::U)))])), 

            ];
            let list = List::new(list_elements)
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");
            list
        };

        let local_app_state_deref = (*app.inner_machine_state).borrow_mut();
        let cpu_local = local_app_state_deref.cpu.clone();
        let registers_list = build_registers_list(&cpu_local);
        f.render_widget(registers_list, chunks[1]);
        let status_list = build_status_view(&cpu_local);
        f.render_widget(status_list, chunks[0]);

    }

    pub fn program_viewer<B: Backend>(f: &mut Frame<B>, app: &App)  {
        let size = Rect::new((f.size().width as f32 * 0.70) as u16, (f.size().height as f32 * 0.31) as u16, (f.size().width as f32 * 0.30) as u16, (f.size().height as f32 * 0.69) as u16);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(5)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(size);

        let block = Block::default().style(Style::default().bg(Color::White).fg(Color::Black));
        f.render_widget(block, size);

        
        let build_program_list = |program_counter: i32, dis: Disassembler| {
            let (counter, start, end) = {
                let program_len = dis.program.len();
                let max_count = 15 as usize;
                if let Some(program_counter) = dis.counters.get(&program_counter) {
    
                    let range_start = program_counter / max_count;
                    let start_region = range_start * max_count;
                    let count = if start_region + max_count >= program_len  {
                        (program_len as i32 - start_region as i32) as usize
                    } else {
                        max_count as usize
                    };
                    (*program_counter, start_region, count)
                } else {
                    (0, 0, program_len.min(max_count))
                }
            };

            let list_elements = dis.program
                .into_iter()
                .enumerate()
                .skip(start as usize)
                .take(end as usize)
                .map(|s| ListItem::new(Spans::from(
                    if s.0 == counter  {
                        vec![Span::raw(format!("> {}", s.1))]
                    } else {
                        vec![Span::raw(format!("  {}", s.1))]
                    }
                )))
                .collect::<Vec<ListItem>>();
            let list = List::new(list_elements)
                .block(Block::default().borders(Borders::ALL).title("Program"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol(">> ");
            list
        };

        let local_app_state_deref = (*app.inner_machine_state).borrow_mut();
        let program = local_app_state_deref.dis.clone();
        let list = build_program_list((local_app_state_deref.cpu.borrow().registers.pc) as i32, program);
        f.render_widget(list, chunks[1]);
    }

    pub fn start(program_path: String) -> Result<(), Error> {
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(it) => it,
            Err(err) => return Err(err),
        };


        let mut app = App {
            memory_page_index: 0,
            inner_machine_state: State::initiate_state(),
            previous_machine_state: Vec::new(),
        };

        terminal.clear()?;
        terminal.hide_cursor()?;
        
        loop {
            terminal.draw(|f| State::build_view(f, &app))?;

            if let Ok(Event::Key(key)) = read() {
                match key.code {
                    KeyCode::PageUp => {
                        app.memory_page_index = (app.memory_page_index + 1) % 0xFF;
                    },
                    KeyCode::PageDown => {
                        if app.memory_page_index == 0 {
                            app.memory_page_index = 0xFF -1;
                        } else {
                            app.memory_page_index = (app.memory_page_index - 1) % 0xFF;
                        }
                    },
                    KeyCode::Enter => {
                        if let Ok(program) = State::load_program_from_file(Some(program_path.clone()))
                        {
                            for (i, byte) in program.iter().enumerate() {
                                app.write((0x8000 + i as u16) as u16, *byte);
                            }

                            let disassembled_program = Disassembler::disassemble(&program);

                            let mut app_state_local_val = (*app.inner_machine_state).borrow_mut();
                            app_state_local_val.dis = disassembled_program;
                        }
                    },
                    KeyCode::Right | KeyCode::Tab => {
                        
                        // add 3 to register A
                        let app_state_local_val = (*app.inner_machine_state).borrow_mut();
                        
                        // deep copy the state
                        let previous_state = app_state_local_val.clone();
                        let mut cpu_local_val = (*app_state_local_val.cpu).borrow_mut();

                        let proceed = {
                            let program_counter = cpu_local_val.registers.pc;
                            app_state_local_val.dis.counters.contains_key(&(program_counter as i32))
                        };
                        
                        if proceed {
                            app.previous_machine_state.push(previous_state);
                            cpu_local_val.tick();
                        }
                    },
                    KeyCode::Left | KeyCode::Backspace => {
                        if let Some(previous_state) = &app.previous_machine_state.pop() {
                            app.inner_machine_state = Rc::new(RefCell::new((*previous_state).clone()));
                            let cpu_ref_local = (*app.inner_machine_state).borrow_mut().cpu.clone();
                            let bus = (*app.inner_machine_state).borrow_mut().bus.clone();
                            (*cpu_ref_local).borrow_mut().bus = Some(bus.clone());
                        }
                    },
                    KeyCode::Insert | KeyCode::Char('i') => {
                        if let Ok(program) = State::load_program_from_file(None)
                        {
                            for (i, byte) in program.iter().enumerate() {
                                app.write((0x8000 + i as u16) as u16, *byte);
                            }

                            let disassembled_program = Disassembler::disassemble(&program);

                            let mut app_state_local_val = (*app.inner_machine_state).borrow_mut();
                            app_state_local_val.dis = disassembled_program;
                        }
                    }

                    KeyCode::Esc | KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

impl DeviceOps for App<'_> {
    fn read(&self, address: u16) -> u8 {
        let local_app_state_deref = (*self.inner_machine_state).borrow_mut();
        let x = local_app_state_deref.bus.borrow_mut().read(address); x
    }

    fn write(&mut self, address: u16, data: u8) {
        let local_app_state_deref = (*self.inner_machine_state).borrow_mut();
        local_app_state_deref.bus.borrow_mut().write(address, data);
    }
}

impl<'a> Clone for State<'a> {
    fn clone(&self) -> Self {
        let cpu_ref = (*self.cpu).borrow();
        State {
            dis: self.dis.clone(),
            cpu: Rc::new(RefCell::new((*cpu_ref).clone())),
            bus: self.bus.deref().borrow().clone_state(),
        }
    }
} 