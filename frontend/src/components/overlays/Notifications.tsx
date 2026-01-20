import { AlertCircle, CheckCircle } from 'lucide-react';
import type { Notification } from '../../types/app';
import { cn } from '../../utils';

type NotificationsProps = {
  notifications: Notification[];
};

export function Notifications({ notifications }: NotificationsProps) {
  if (notifications.length === 0) return null;

  return (
    <div className="fixed right-6 top-6 z-50 flex flex-col gap-3">
      {notifications.map((notification) => (
        <div
          key={notification.id}
          className={cn(
            'notification',
            notification.type === 'success' ? 'notification-success' : 'notification-error'
          )}
        >
          {notification.type === 'success' ? (
            <CheckCircle className="h-5 w-5" />
          ) : (
            <AlertCircle className="h-5 w-5" />
          )}
          <span>{notification.message}</span>
        </div>
      ))}
    </div>
  );
}
