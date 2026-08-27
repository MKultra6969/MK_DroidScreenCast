import { memo } from 'react';
import { CircleAlert, CircleCheck } from 'lucide-react';
import type { Notification } from '../../types/app';
import { cn } from '../../utils';

type NotificationsProps = {
  notifications: Notification[];
};

function NotificationsView({ notifications }: NotificationsProps) {
  if (notifications.length === 0) return null;

  return (
    <div
      className="pointer-events-none fixed right-6 top-6 z-[1400] flex flex-col items-end gap-2.5"
      role="status"
      aria-live="polite"
    >
      {notifications.map((notification) => (
        <div
          key={notification.id}
          className={cn(
            'm3-snackbar pointer-events-auto',
            notification.type === 'success' ? 'm3-snackbar--success' : 'm3-snackbar--error'
          )}
        >
          {notification.type === 'success' ? <CircleCheck /> : <CircleAlert />}
          <span className="min-w-0 flex-1">{notification.message}</span>
        </div>
      ))}
    </div>
  );
}

export const Notifications = memo(NotificationsView);
