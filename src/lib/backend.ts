import { invoke } from '@tauri-apps/api/core';

export interface AgentRecord {
  id: string;
  name: string;
  instructions?: string | null;
  status: 'idle' | 'running' | 'stopped';
  model: string;
  tools: string[];
  createdAt: string;
  updatedAt: string;
}

export const agentList = () => invoke<AgentRecord[]>('agent_list');
