import { useSession } from './state/session';
import { useKeyboardShortcuts } from './state/keyboard';
import { TitleBar } from './components/titlebar/TitleBar';
import { TransportBar } from './components/transport/TransportBar';
import { BrowserPanel } from './components/browser/BrowserPanel';
import { ArrangementToolbar } from './components/arrangement/ArrangementToolbar';
import { Arrangement } from './components/arrangement/Arrangement';
import { EditorPane } from './components/editor/EditorPane';
import { InspectorPanel } from './components/inspector/InspectorPanel';
import { AgentPanel } from './components/agent/AgentPanel';
import { AgentRail } from './components/agent/AgentRail';
import styles from './App.module.css';

export function App() {
  useKeyboardShortcuts();
  const agentOpen = useSession((s) => s.view.agentPanelOpen);
  return (
    <div className={styles.window}>
      <TitleBar />
      <TransportBar />
      <div className={styles.body}>
        <BrowserPanel />
        <div className={styles.center}>
          <ArrangementToolbar />
          <Arrangement />
          <EditorPane />
        </div>
        <InspectorPanel />
        {agentOpen ? <AgentPanel /> : <AgentRail />}
      </div>
    </div>
  );
}
