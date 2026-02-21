import { describe, it, expect } from 'vitest';
import { globMatch } from './glob-match';

describe('globMatch', () => {
  it('matches exact string', () => {
    expect(globMatch('Agent', 'Agent')).toBe(true);
  });

  it('is case-insensitive', () => {
    expect(globMatch('agent *', 'Agent Foo')).toBe(true);
    expect(globMatch('AGENT *', 'agent bar')).toBe(true);
  });

  it('matches trailing wildcard', () => {
    expect(globMatch('Agent *', 'Agent foo')).toBe(true);
    expect(globMatch('Agent *', 'Agent')).toBe(false);
  });

  it('matches leading wildcard', () => {
    expect(globMatch('*-orchestrator', 'team-orchestrator')).toBe(true);
    expect(globMatch('*-orchestrator', 'orchestrator')).toBe(false);
  });

  it('matches wildcard in the middle', () => {
    expect(globMatch('test-*-workspace', 'test-foo-workspace')).toBe(true);
    expect(globMatch('test-*-workspace', 'test--workspace')).toBe(true);
  });

  it('matches multiple wildcards', () => {
    expect(globMatch('*agent*', 'my-agent-workspace')).toBe(true);
    expect(globMatch('*agent*', 'agent')).toBe(true);
  });

  it('matches single-character wildcard (?)', () => {
    expect(globMatch('Agent ?', 'Agent A')).toBe(true);
    expect(globMatch('Agent ?', 'Agent AB')).toBe(false);
  });

  it('does not match when pattern does not apply', () => {
    expect(globMatch('Agent *', 'Default')).toBe(false);
    expect(globMatch('foo', 'bar')).toBe(false);
  });

  it('escapes regex special characters in pattern', () => {
    expect(globMatch('file.txt', 'file.txt')).toBe(true);
    expect(globMatch('file.txt', 'fileTtxt')).toBe(false);
    expect(globMatch('a+b', 'a+b')).toBe(true);
    expect(globMatch('(test)', '(test)')).toBe(true);
  });

  it('handles empty pattern and text', () => {
    expect(globMatch('', '')).toBe(true);
    expect(globMatch('*', '')).toBe(true);
    expect(globMatch('', 'notempty')).toBe(false);
  });

  it('star matches empty substring', () => {
    expect(globMatch('*', 'anything')).toBe(true);
    expect(globMatch('prefix*', 'prefix')).toBe(true);
  });
});
