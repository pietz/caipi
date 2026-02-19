// Files state store using Svelte 5 runes
import { SvelteSet } from 'svelte/reactivity';

export interface FileEntry {
  name: string;
  type: 'file' | 'folder';
  path: string;
  children?: FileEntry[];
}

class FilesState {
  rootPath = $state<string | null>(null);
  tree = $state<FileEntry[]>([]);
  expanded = $state(new SvelteSet<string>());
  selected = $state<string | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  setRootPath(path: string) {
    this.rootPath = path;
  }

  setTree(entries: FileEntry[]) {
    this.tree = entries;
    this.loading = false;
  }

  setLoading(loading: boolean) {
    this.loading = loading;
  }

  setError(error: string | null) {
    this.error = error;
    this.loading = false;
  }

  setSelected(path: string | null) {
    this.selected = path;
  }

  toggleExpanded(path: string) {
    const newExpanded = new SvelteSet(this.expanded);
    if (newExpanded.has(path)) {
      newExpanded.delete(path);
    } else {
      newExpanded.add(path);
    }
    this.expanded = newExpanded;
  }

  setExpanded(path: string, expanded: boolean) {
    const newSet = new SvelteSet(this.expanded);
    if (expanded) {
      newSet.add(path);
    } else {
      newSet.delete(path);
    }
    this.expanded = newSet;
  }

  updateChildren(parentPath: string, children: FileEntry[]) {
    const updateNode = (nodes: FileEntry[]): FileEntry[] => {
      return nodes.map(node => {
        if (node.path === parentPath) {
          return { ...node, children };
        }
        if (node.children) {
          return { ...node, children: updateNode(node.children) };
        }
        return node;
      });
    };
    this.tree = updateNode(this.tree);
  }

  reset() {
    this.rootPath = null;
    this.tree = [];
    this.expanded = new SvelteSet();
    this.selected = null;
    this.loading = false;
    this.error = null;
  }
}

export const files = new FilesState();
