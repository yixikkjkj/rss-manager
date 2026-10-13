import type { FolderVO } from '../Types';
/** A shared path format for tree menus, moving feeds and selecting article favorites. */
export function folderPaths(folders: FolderVO[]) {
  const byId = new Map(folders.map((folder) => [folder.id, folder]));
  return folders.map((folder) => {
    const names = [folder.name];
    const seen = new Set([folder.id]);
    let parent = folder.parent_id;
    while (parent !== null && !seen.has(parent)) {
      seen.add(parent);
      const node = byId.get(parent);
      if (!node) break;
      names.unshift(node.name);
      parent = node.parent_id;
    }
    return { ...folder, path: names.join(' / ') };
  });
}
export function descendantIds(folders: FolderVO[], id: number): Set<number> {
  const ids = new Set([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const folder of folders)
      if (folder.parent_id !== null && ids.has(folder.parent_id) && !ids.has(folder.id)) {
        ids.add(folder.id);
        changed = true;
      }
  }
  return ids;
}
