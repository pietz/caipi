import { writable, derived } from 'svelte/store';

export interface FileEntry {
  name: string;
  type: 'file' | 'folder';
  path: string;
  children?: FileEntry[];
}

export interface FilesState {
  rootPath: string | null;
  tree: FileEntry[];
  expandedPaths: Set<string>;
  selectedPath: string | null;
  loading: boolean;
  error: string | null;
}

const initialState: FilesState = {
  rootPath: null,
  tree: [],
  expandedPaths: new Set(),
  selectedPath: null,
  loading: false,
  error: null,
};

function createFilesStore() {
  const { subscribe, set, update } = writable<FilesState>(initialState);

  return {
    subscribe,

    setRootPath: (path: string) => update(s => ({
      ...s,
      rootPath: path,
    })),

    setTree: (tree: FileEntry[]) => update(s => ({
      ...s,
      tree,
      loading: false,
    })),

    setLoading: (loading: boolean) => update(s => ({
      ...s,
      loading,
    })),

    setError: (error: string | null) => update(s => ({
      ...s,
      error,
      loading: false,
    })),

    setSelectedPath: (path: string | null) => update(s => ({
      ...s,
      selectedPath: path,
    })),

    toggleExpanded: (path: string) => update(s => {
      const newExpanded = new Set(s.expandedPaths);
      if (newExpanded.has(path)) {
        newExpanded.delete(path);
      } else {
        newExpanded.add(path);
      }
      return {
        ...s,
        expandedPaths: newExpanded,
      };
    }),

    setExpanded: (path: string, expanded: boolean) => update(s => {
      const newExpanded = new Set(s.expandedPaths);
      if (expanded) {
        newExpanded.add(path);
      } else {
        newExpanded.delete(path);
      }
      return {
        ...s,
        expandedPaths: newExpanded,
      };
    }),

    updateChildren: (parentPath: string, children: FileEntry[]) => update(s => {
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
      return {
        ...s,
        tree: updateNode(s.tree),
      };
    }),

    reset: () => set(initialState),
  };
}

export const filesStore = createFilesStore();

// Derived stores
export const fileTree = derived(filesStore, $files => $files.tree);
export const selectedFile = derived(filesStore, $files => $files.selectedPath);
export const isFilesLoading = derived(filesStore, $files => $files.loading);
