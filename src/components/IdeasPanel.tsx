import { ArrowRight, CheckCircle2, Lightbulb, MoreHorizontal, Plus } from "lucide-react";
import { FormEvent, useState } from "react";
import type { Idea } from "../types";

type Props = {
  ideas: Idea[];
  onAdd: (title: string) => void;
  onAccept: (id: string) => void;
  onConvert: (idea: Idea) => void;
};

const statusLabels: Record<Idea["status"], string> = {
  pending: "待整理",
  doing: "处理中",
  accepted: "已采纳",
  converted: "已转待办",
  archived: "已归档",
};

export function IdeasPanel({ ideas, onAdd, onAccept, onConvert }: Props) {
  const [draft, setDraft] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const title = draft.trim();
    if (!title) return;
    onAdd(title);
    setDraft("");
  };

  return (
    <section className="side-section ideas-section" aria-labelledby="ideas-title">
      <div className="side-section-heading">
        <div>
          <h2 id="ideas-title">意见整理</h2>
          <span>快速记录想法与反馈</span>
        </div>
        <span className="idea-count">{ideas.filter((idea) => idea.status === "pending").length}</span>
      </div>

      <form className="idea-capture" onSubmit={submit}>
        <Lightbulb size={17} />
        <input value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="记录一个想法…" aria-label="添加意见" />
        <button type="submit" aria-label="添加意见">
          <Plus size={16} />
        </button>
      </form>

      <div className="idea-list">
        {ideas.slice(0, 3).map((idea) => (
          <article className="idea-item" key={idea.id}>
            <div className="idea-topline">
              <span className={`idea-status status-${idea.status}`}>
                {idea.status === "accepted" ? <CheckCircle2 size={12} /> : null}
                {statusLabels[idea.status]}
              </span>
              <button aria-label="意见更多操作"><MoreHorizontal size={15} /></button>
            </div>
            <strong>{idea.title}</strong>
            <div className="idea-footer">
              <div className="tag-list">
                {idea.tags.slice(0, 2).map((tag) => <span key={tag}>{tag}</span>)}
              </div>
              {idea.status === "pending" ? (
                <div className="idea-actions">
                  <button onClick={() => onAccept(idea.id)}>采纳</button>
                  <button className="convert-action" onClick={() => onConvert(idea)} title="转为待办">
                    <ArrowRight size={13} />
                  </button>
                </div>
              ) : null}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
