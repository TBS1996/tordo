use std::{fs, io};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use nonempty::NonEmpty;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::{fmt::Display};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use ratatui::widgets::{Block, Borders, List, ListItem, Widget};
use reformy::FormRenderable;


struct App {
    ledger: Ledger<Task>,
}

impl App {
    fn tasks(&self) -> Vec<Task> {
        let mut done: Vec<Task> = vec![];
        let mut todo: Vec<Task> = vec![];
        let mut in_progress: Vec<Task> = vec![];
        let mut suspended: Vec<Task> = vec![];
        let mut blocked: Vec<Task> = vec![];
        let tasks = self.ledger.load_all();
        let task_qty = tasks.len();
        for task  in tasks {
            let status = task.status;
            match status {
                Status::Blocked => {
                    blocked.push(task);
                }
                Status::Todo => {
                    todo.push(task);
                },
                Status::Inprogress => {
                    in_progress.push(task);
                },
                Status::Done => {
                    done.push(task);
                },
                Status::Suspended => {
                    suspended.push(task);
                },
            }
        }

        let mut out = Vec::with_capacity(task_qty);
        out.extend(in_progress);
        out.extend(blocked);
        out.extend(todo);
        out.extend(suspended);
        out.extend(done);

        out

    }
}


fn main() {
    run();
}

fn run() -> Option<()> {
    let root = dirs::data_dir().unwrap().join("tordo");
    fs::create_dir_all(&root).unwrap();
    let ledger: Ledger<Task> = Ledger::new(root);

    let app = App {
        ledger
    };

    let  items = app.tasks();

    if items.is_empty() {
        let action = ledgerstore::TheLedgerAction::Create(Task::new("new task".to_string()));
        app.ledger.modify(TheLedgerEvent::new(TaskID::new_v4(), action)).unwrap();
    }

    loop {
        let items: Vec<Task> = app.tasks();
        if let Ok(Some(act)) = run_selection_menu(items){
            match act {
                SelAct::Modify(item) => {
                    if let Some(action)  = bruhmain() {
                        let action = ledgerstore::TheLedgerAction::Modify(action);
                        app.ledger.modify(TheLedgerEvent::new(item.id, action)).unwrap();
                    }
                },
                SelAct::Create => {
                    let action = ledgerstore::TheLedgerAction::Create(Task::new("new task".to_string()));
                    app.ledger.modify(TheLedgerEvent::new(TaskID::new_v4(), action)).unwrap();
                },
                SelAct::Delete(item) => {
                    app.ledger.modify(TheLedgerEvent::new_delete(item.id)).unwrap();
                },
            }
        } else {
            break;
        }
    }


    Some(())
}

enum SelAct<T> {
    Modify(T),
    Create,
    Delete(T),
}

fn run_selection_menu<T: Display>(items: Vec<T>) -> io::Result<Option<SelAct<T>>>
where
    T: Clone,
{
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut selected = 0;
    let result = loop {
        terminal.draw(|f| {
            let size = f.area();
            let items_widget = List::new(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let prefix = if i == selected { "> " } else { "  " };
                        ListItem::new(format!("{prefix}{item}"))
                    })
                    .collect::<Vec<_>>(),
            )
            .block(Block::default().title("Choose item").borders(Borders::ALL));

            f.render_widget(items_widget, size);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('n') => break Some(SelAct::Create),
                    KeyCode::Up | KeyCode::Char('k') => {
                        if selected > 0 {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j')=> {
                        if selected + 1 < items.len() {
                            selected += 1;
                        }
                    }
                    KeyCode::Delete => break Some(SelAct::Delete(items[selected].clone())),
                    KeyCode::Enter => break Some(SelAct::Modify(items[selected].clone())),
                    KeyCode::Esc => break None,
                    _ => {}
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(result)
}



fn bruhmain() -> Option<TaskAction> {
    let mut foo = TaskAction::form();
    let mut terminal = ratatui::init();

    loop {
        terminal
            .draw(|f| {
                f.render_widget(&foo, f.area());
            })
            .unwrap();

        if let Event::Key(key) = event::read().unwrap() {
            match key.code {
                event::KeyCode::Esc => break,
                key => {
                    let input = tui_textarea::Input {
                        key: key.into(),
                        ..Default::default()
                    };
                    foo.input(input);
                }
            }
        }
    }

    ratatui::restore();
    foo.build()
}


use ledgerstore::{LedgerItem, TheLedgerEvent};
use ledgerstore::Ledger;

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", &self.name, &self.status )
    }
}

impl LedgerItem for Task {
    type Key = TaskID;

    type Error = ();

    type RefType = String;

    type PropertyType = String;

    type Modifier = TaskAction;

    fn inner_run_event(mut self, event: Self::Modifier) -> Result<Self, Self::Error> {
        match event {
            TaskAction::SetStatus{status} => {
                self.status = status;
            },
            TaskAction::SetName{name} => {
                self.name = name;
            },
        }

        Ok(self)
    }

    fn new_default(id: Self::Key) -> Self {
        Self {
            name: "uninit".to_string(),
            id,
            //dependencies: BTreeSet::default(),
            status: Status::Todo,
        }
    }

    fn item_id(&self) -> Self::Key {
        self.id
    }
}

type TaskID = Uuid;

#[derive(Clone, Serialize, Deserialize, Hash, Debug, FormRenderable)]
enum TaskAction {
    SetStatus{
        #[form(nested)]
        status: Status
    },
    SetName{
        name: String,
    },
}


#[derive(Eq, PartialEq, Clone, Debug, Hash, Deserialize, Serialize, FormRenderable)]
struct Task {
    name: String,
    id: TaskID,
    //dependencies: BTreeSet<TaskID>,
    #[form(nested)]
    status: Status,
}

#[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Debug)]
enum TaskType {
    Leaf {
        status: Status,
    },
    Epic {
        sub_tasks: NonEmpty<TaskID>,
    }
}

/// Importance is defined on the root
/// time estimate is defined on the leaves
///
/// Priority is defined as importance divided by time   
enum TheTaskType {
    Leaf {
        time_estimate: f32,
        status: Status,
    },
    Epic {
        importance: f32,
        sub_tasks: NonEmpty<TaskID>,
    },
    SubEpic {
        sub_tasks: NonEmpty<TaskID>,
    },
    Single {
        time_estimate: f32,
        importance: f32,
        status: Status,
    }
}


impl Task {
    fn new(name: String) -> Self {
        Self {
            name,
            id: Uuid::new_v4(),
            //dependencies: Default::default(),
            status: Status::Todo,
        }
    }
}

#[derive(Default, Eq, PartialEq, PartialOrd, Ord, Clone, Debug, Hash, Deserialize, Serialize, FormRenderable, Copy)]
enum Status {
    #[default]
    Todo,
    Inprogress,
    Done,
    Suspended,
    Blocked,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

