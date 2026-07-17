import { ArrowUp, Mic, MicOff, Sparkles, Volume2 } from "lucide-react";
import { FormEvent, useState } from "react";

type Props = {
  message: string;
  isListening: boolean;
  onSubmit: (text: string) => void;
  onMic: () => void;
};

export function AssistantDock({ message, isListening, onSubmit, onMic }: Props) {
  const [draft, setDraft] = useState("");

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const text = draft.trim();
    if (!text) return;
    onSubmit(text);
    setDraft("");
  };

  return (
    <section className="assistant-dock" aria-label="本地桌面助手">
      <div className="assistant-message">
        <span className="assistant-status"><Sparkles size={13} /> 本地助手</span>
        <p>{message}</p>
      </div>

      <div className={`pet-stage ${isListening ? "is-listening" : ""}`} onClick={onMic} role="button" tabIndex={0} aria-label="点击桌面宠物开始语音">
        <div className="pet-shadow" />
        <div className="pet-body">
          <div className="pet-tail" />
          <div className="pet-head">
            <div className="pet-ear pet-ear-left"><i /></div>
            <div className="pet-ear pet-ear-right"><i /></div>
            <div className="pet-face">
              <span className="pet-eye pet-eye-left" />
              <span className="pet-eye pet-eye-right" />
              <span className="pet-mouth" />
            </div>
            <span className="pet-cheek pet-cheek-left" />
            <span className="pet-cheek pet-cheek-right" />
          </div>
          <div className="pet-chest"><Volume2 size={12} /></div>
          <div className="pet-paw pet-paw-left" />
          <div className="pet-paw pet-paw-right" />
        </div>
        <div className="listening-ring ring-one" />
        <div className="listening-ring ring-two" />
      </div>

      <form className="assistant-input" onSubmit={submit}>
        <button type="button" className={isListening ? "is-live" : ""} onClick={onMic} aria-label="语音输入">
          {isListening ? <MicOff size={17} /> : <Mic size={17} />}
        </button>
        <input value={draft} onChange={(event) => setDraft(event.target.value)} placeholder="输入：打开项目报告" aria-label="桌面助手指令" />
        <button type="submit" className="send-command" aria-label="发送指令">
          <ArrowUp size={16} />
        </button>
      </form>
    </section>
  );
}
