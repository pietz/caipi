/**
 * Permission Coordinator
 *
 * Handles permission-related interactions including keyboard shortcuts
 * and communication with the backend for permission responses.
 */

import { invoke } from '@tauri-apps/api/core';
import { chatStore, type PermissionRequest, type StreamItem } from '$lib/stores';

export interface PermissionCoordinatorOptions {
  getSessionId: () => string | null;
  getStreamItems: () => StreamItem[];
  getPendingPermissions: () => Record<string, PermissionRequest>;
}

/**
 * Create a permission coordinator for handling permission interactions
 */
export function createPermissionCoordinator(options: PermissionCoordinatorOptions) {
  const { getSessionId, getStreamItems, getPendingPermissions } = options;

  /**
   * Set up global keyboard shortcuts for permission handling.
   * Returns a cleanup function to remove the listener.
   */
  function setupKeyboardShortcuts(): () => void {
    function handleKeydown(e: KeyboardEvent) {
      const pendingPermissions = getPendingPermissions();
      const permissionKeys = Object.keys(pendingPermissions);
      if (permissionKeys.length === 0) return;

      if (e.key === 'Enter' || e.key === 'Escape') {
        const activeElement = document.activeElement as HTMLTextAreaElement | null;
        const isTextareaWithContent =
          activeElement?.tagName === 'TEXTAREA' && activeElement.value.trim().length > 0;

        if (isTextareaWithContent) return;

        e.preventDefault();

        // Find the first pending permission by activity order in streamItems
        const streamItems = getStreamItems();
        const sortedItems = [...streamItems].sort((a, b) => a.insertionIndex - b.insertionIndex);
        const firstPendingActivity = sortedItems.find(
          (item) => item.type === 'tool' && item.activity && pendingPermissions[item.activity.id]
        );

        if (firstPendingActivity?.activity) {
          const permission = pendingPermissions[firstPendingActivity.activity.id];
          if (permission) {
            const allowed = e.key === 'Enter';
            respondToPermission(permission, allowed);
          }
        }
      }
    }

    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  }

  /**
   * Respond to a permission request
   */
  async function respondToPermission(permission: PermissionRequest, allowed: boolean): Promise<void> {
    const sessionId = getSessionId();
    if (!sessionId) return;

    try {
      await invoke('respond_permission', {
        sessionId,
        requestId: permission.id,
        allowed,
      });
    } catch (e) {
      console.error('Failed to respond to permission:', e);
    }

    // Remove this specific permission request
    const key = permission.activityId || permission.id;
    chatStore.removePermissionRequest(key);
  }

  /**
   * Respond to a permission by activity ID (convenience wrapper)
   */
  function respondToPermissionByActivityId(activityId: string, allowed: boolean): void {
    const pendingPermissions = getPendingPermissions();
    const permission = pendingPermissions[activityId];
    if (permission) {
      respondToPermission(permission, allowed);
    }
  }

  return {
    setupKeyboardShortcuts,
    respondToPermission,
    respondToPermissionByActivityId,
  };
}
