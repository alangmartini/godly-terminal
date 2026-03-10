import { writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import type { RunResult, StepResult } from './types.js';

export class ArtifactBundle {
  private dir: string;
  private files: string[] = [];

  constructor(baseDir: string, runId: string) {
    this.dir = join(baseDir, runId);
    if (!existsSync(this.dir)) {
      mkdirSync(this.dir, { recursive: true });
    }
  }

  get artifactDir(): string {
    return this.dir;
  }

  writeManifest(contractId: string): void {
    const manifest = {
      run_id: this.dir.split(/[/\\]/).pop(),
      contract_id: contractId,
      created_at: new Date().toISOString(),
      artifact_dir: this.dir,
      files: this.files,
    };
    this.writeFile('manifest.json', JSON.stringify(manifest, null, 2));
  }

  writeResult(result: RunResult): void {
    this.writeFile('result.json', JSON.stringify(result, null, 2));
  }

  writeStepTrace(stepId: string, stepResult: StepResult): void {
    const filename = `steps/${stepId}.json`;
    this.writeFile(filename, JSON.stringify(stepResult, null, 2));
  }

  writeFile(filename: string, content: string): void {
    const filepath = join(this.dir, filename);
    const dir = dirname(filepath);
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    writeFileSync(filepath, content, 'utf-8');
    if (!this.files.includes(filename)) {
      this.files.push(filename);
    }
  }

  finalize(): string[] {
    return [...this.files];
  }
}
