import { ArrowUpRight, Check, Circle, Clock3, Plus } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Todo } from "../types";

type Props = {
  todos: Todo[];
  onAdd: (title: string) => void;
  onToggle: (id: string) => void;
  onAction: (todo: Todo) => void;
};

export function TodoPanel({ todos, onAdd, onToggle, onAction }: Props) {
  const [draft, setDraft] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const title = draft.trim();
    if (!title) return;
    onAdd(title);
    setDraft("");
  };

  return (
    <section className="side-section todo-section" aria-labelledby="todos-title">
      <div className="side-section-heading">
        <div>
          <h2 id="todos-title">今天待办</h2>
          <span>{todos.filter((todo) => todo.status !== "done").length} 项未完成</span>
        </div>
        <button className="tiny-action" onClick={() => document.getElementById("new-todo")?.focus()}>
          <Plus size={15} />
          新增
        </button>
      </div>

      <div className="todo-list">
        {todos.slice(0, 4).map((todo) => (
          <article className={`todo-item ${todo.status === "done" ? "is-complete" : ""}`} key={todo.id}>
            <button className="todo-check" onClick={() => onToggle(todo.id)} aria-label={todo.status === "done" ? "恢复待办" : "完成待办"}>
              {todo.status === "done" ? <Check size={14} /> : <Circle size={15} />}
            </button>
            <button className="todo-copy" onClick={() => todo.actionType && onAction(todo)}>
              <strong>{todo.title}</strong>
              <span>
                <Clock3 size={12} />
                {todo.time}
                <i className={`priority-dot priority-${todo.priority}`} />
                {todo.status === "doing" ? "进行中" : todo.status === "done" ? "已完成" : "待开始"}
              </span>
            </button>
            {todo.actionType ? (
              <button className="todo-action" onClick={() => onAction(todo)} aria-label="执行待办动作">
                <ArrowUpRight size={15} />
              </button>
            ) : null}
          </article>
        ))}
      </div>

      <form className="quick-add" onSubmit={submit}>
        <Plus size={16} />
        <input id="new-todo" value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="新增待办…" />
        <button type="submit">添加</button>
      </form>
    </section>
  );
}
