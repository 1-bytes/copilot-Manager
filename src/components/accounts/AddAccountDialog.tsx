import { useState, useEffect, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { Plus, Loader2, CheckCircle2, XCircle, Copy, Check, Key, ExternalLink, Timer, Github } from 'lucide-react';
import { useAccountStore } from '../../stores/useAccountStore';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../../utils/request';
import { copyToClipboard } from '../../utils/clipboard';

interface AddAccountDialogProps {
    onAdd: (email: string, token: string) => Promise<void>;
    showText?: boolean;
}

type Status = 'idle' | 'loading' | 'success' | 'error';

interface DeviceFlowInfo {
    user_code: string;
    verification_uri: string;
    expires_in: number;
}

type DeviceFlowState =
    | { step: 'idle' }
    | { step: 'starting' }
    | { step: 'waiting'; info: DeviceFlowInfo; startedAt: number }
    | { step: 'polling' }
    | { step: 'success' }
    | { step: 'error'; message: string };

function AddAccountDialog({ showText = true }: AddAccountDialogProps) {
    const { t } = useTranslation();
    const fetchAccounts = useAccountStore(state => state.fetchAccounts);
    const [isOpen, setIsOpen] = useState(false);
    const [activeTab, setActiveTab] = useState<'device' | 'token'>('device');
    const [githubToken, setGithubToken] = useState('');
    const [userCodeCopied, setUserCodeCopied] = useState(false);

    // UI State
    const [status, setStatus] = useState<Status>('idle');
    const [message, setMessage] = useState('');

    // Device Flow state
    const [deviceFlow, setDeviceFlow] = useState<DeviceFlowState>({ step: 'idle' });
    const [countdown, setCountdown] = useState(0);
    const pollAbortRef = useRef(false);
    const countdownIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

    // Reset state when dialog opens or tab changes
    useEffect(() => {
        if (isOpen) {
            resetState();
        }
    }, [isOpen, activeTab]);

    // Cleanup on unmount or dialog close
    useEffect(() => {
        return () => {
            pollAbortRef.current = true;
            if (countdownIntervalRef.current) {
                clearInterval(countdownIntervalRef.current);
            }
        };
    }, []);

    // Countdown timer for device flow
    useEffect(() => {
        if (deviceFlow.step === 'waiting') {
            const { info, startedAt } = deviceFlow;
            const expiresAt = startedAt + info.expires_in;

            const updateCountdown = () => {
                const now = Math.floor(Date.now() / 1000);
                const remaining = expiresAt - now;
                if (remaining <= 0) {
                    setCountdown(0);
                    setDeviceFlow({ step: 'error', message: t('accounts.add.device.expired', 'Device code expired. Please start again.') });
                    if (countdownIntervalRef.current) {
                        clearInterval(countdownIntervalRef.current);
                        countdownIntervalRef.current = null;
                    }
                } else {
                    setCountdown(remaining);
                }
            };

            updateCountdown();
            countdownIntervalRef.current = setInterval(updateCountdown, 1000);

            return () => {
                if (countdownIntervalRef.current) {
                    clearInterval(countdownIntervalRef.current);
                    countdownIntervalRef.current = null;
                }
            };
        }
    }, [deviceFlow, t]);

    const resetState = () => {
        setStatus('idle');
        setMessage('');
        setGithubToken('');
        setUserCodeCopied(false);
        setDeviceFlow({ step: 'idle' });
        setCountdown(0);
        pollAbortRef.current = true;
        if (countdownIntervalRef.current) {
            clearInterval(countdownIntervalRef.current);
            countdownIntervalRef.current = null;
        }
    };

    const formatCountdown = (seconds: number): string => {
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${mins}:${secs.toString().padStart(2, '0')}`;
    };

    // ── Device Flow ──────────────────────────────────────────────────

    const handleStartDeviceFlow = useCallback(async () => {
        setDeviceFlow({ step: 'starting' });
        setStatus('idle');
        setMessage('');
        pollAbortRef.current = false;

        try {
            const info = await invoke<DeviceFlowInfo>('start_device_flow');
            const startedAt = Math.floor(Date.now() / 1000);
            setDeviceFlow({ step: 'waiting', info, startedAt });

            // Auto-start polling after showing the code
            handlePollDeviceFlow();
        } catch (error) {
            const errMsg = String(error);
            setDeviceFlow({ step: 'error', message: errMsg });
            setStatus('error');
            setMessage(`${t('common.error')}: ${errMsg}`);
        }
    }, [t]);

    const handlePollDeviceFlow = useCallback(async () => {
        try {
            await invoke<any>('complete_device_flow');
            if (pollAbortRef.current) return;

            setDeviceFlow({ step: 'success' });
            setStatus('success');
            setMessage(t('accounts.add.device.success', 'Account added successfully!'));

            await fetchAccounts();

            setTimeout(() => {
                setIsOpen(false);
                resetState();
            }, 1500);
        } catch (error) {
            if (pollAbortRef.current) return;

            const errMsg = String(error);
            setDeviceFlow({ step: 'error', message: errMsg });
            setStatus('error');
            setMessage(`${t('common.error')}: ${errMsg}`);
        }
    }, [fetchAccounts, t]);

    const handleCancelDeviceFlow = useCallback(async () => {
        pollAbortRef.current = true;
        try {
            await invoke('cancel_device_flow');
        } catch {
            // Ignore cancel errors
        }
        setDeviceFlow({ step: 'idle' });
        setStatus('idle');
        setMessage('');
    }, []);

    const handleCopyUserCode = async (code: string) => {
        const success = await copyToClipboard(code);
        if (success) {
            setUserCodeCopied(true);
            setTimeout(() => setUserCodeCopied(false), 2000);
        }
    };

    const handleOpenVerification = (uri: string) => {
        window.open(uri, '_blank');
    };

    // ── Token Tab ────────────────────────────────────────────────────

    const handleTokenSubmit = async () => {
        const token = githubToken.trim();
        if (!token) {
            setStatus('error');
            setMessage(t('accounts.add.token.error_empty', 'Please enter a GitHub token.'));
            return;
        }

        setStatus('loading');
        setMessage(t('accounts.add.token.adding', 'Adding account...'));

        // Parse: support single token or JSON array of { github_token: "..." }
        let tokens: string[] = [];

        try {
            if (token.startsWith('[') && token.endsWith(']')) {
                const parsed = JSON.parse(token);
                if (Array.isArray(parsed)) {
                    tokens = parsed
                        .map((item: any) => {
                            if (typeof item === 'string') return item;
                            return item.github_token || item.token || '';
                        })
                        .filter((t: string) => t.length > 0);
                }
            }
        } catch {
            // Not JSON, treat as raw token(s)
        }

        // If JSON parsing didn't yield results, split by newlines/commas
        if (tokens.length === 0) {
            tokens = token
                .split(/[\n,]+/)
                .map(t => t.trim())
                .filter(t => t.length > 0);
        }

        // Deduplicate
        tokens = [...new Set(tokens)];

        if (tokens.length === 0) {
            setStatus('error');
            setMessage(t('accounts.add.token.error_empty', 'Please enter a GitHub token.'));
            return;
        }

        // Batch add
        let successCount = 0;
        let failCount = 0;

        for (let i = 0; i < tokens.length; i++) {
            const currentToken = tokens[i];
            if (tokens.length > 1) {
                setMessage(t('accounts.add.token.batch_progress', { current: i + 1, total: tokens.length }));
            }

            try {
                await invoke('add_account', { email: '', githubToken: currentToken });
                successCount++;
            } catch (error) {
                console.error(`Failed to add token ${i + 1}:`, error);
                failCount++;
            }

            if (tokens.length > 1) {
                await new Promise(r => setTimeout(r, 100));
            }
        }

        await fetchAccounts();

        if (successCount === tokens.length) {
            setStatus('success');
            if (tokens.length === 1) {
                setMessage(t('accounts.add.device.success', 'Account added successfully!'));
            } else {
                setMessage(t('accounts.add.token.batch_success', { count: successCount }));
            }
            setTimeout(() => {
                setIsOpen(false);
                resetState();
            }, 1500);
        } else if (successCount > 0) {
            setStatus('success');
            setMessage(t('accounts.add.token.batch_partial', { success: successCount, fail: failCount }));
        } else {
            setStatus('error');
            setMessage(t('accounts.add.token.batch_fail', 'All tokens failed to add.'));
        }
    };

    // ── Status Alert ─────────────────────────────────────────────────

    const StatusAlert = () => {
        if (status === 'idle' || !message) return null;

        const styles = {
            loading: 'alert-info',
            success: 'alert-success',
            error: 'alert-error'
        };

        const icons = {
            loading: <Loader2 className="w-5 h-5 animate-spin" />,
            success: <CheckCircle2 className="w-5 h-5" />,
            error: <XCircle className="w-5 h-5" />
        };

        return (
            <div className={`alert ${styles[status]} mb-4 text-sm py-2 shadow-sm`}>
                {icons[status]}
                <span>{message}</span>
            </div>
        );
    };

    // ── Render ───────────────────────────────────────────────────────

    return (
        <>
            <button
                className="px-2.5 lg:px-4 py-2 bg-white dark:bg-base-100 text-gray-700 dark:text-gray-300 text-sm font-medium rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 transition-colors flex items-center gap-2 shadow-sm border border-gray-200/50 dark:border-base-300 relative z-[100]"
                onClick={() => setIsOpen(true)}
                title={!showText ? t('accounts.add_account') : undefined}
            >
                <Plus className="w-4 h-4" />
                {showText && <span className="hidden lg:inline">{t('accounts.add_account')}</span>}
            </button>

            {isOpen && createPortal(
                <div
                    className="fixed inset-0 z-[99999] flex items-center justify-center bg-black/50 backdrop-blur-sm"
                    style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0 }}
                >
                    {/* Draggable Top Region */}
                    <div data-tauri-drag-region className="fixed top-0 left-0 right-0 h-8 z-[1]" />

                    {/* Click outside to close */}
                    <div className="absolute inset-0 z-[0]" onClick={() => {
                        if (deviceFlow.step === 'waiting') {
                            handleCancelDeviceFlow();
                        }
                        setIsOpen(false);
                    }} />

                    <div className="bg-white dark:bg-base-100 text-gray-900 dark:text-base-content rounded-2xl shadow-2xl w-full max-w-lg p-6 relative z-[10] m-4 max-h-[90vh] overflow-y-auto">
                        <h3 className="font-bold text-lg mb-4">{t('accounts.add.title')}</h3>

                        {/* Tab Navigation */}
                        <div className="bg-gray-100 dark:bg-base-200 p-1 rounded-xl mb-6 grid grid-cols-2 gap-1">
                            <button
                                className={`py-2 px-3 rounded-lg text-sm font-medium transition-all duration-200 ${activeTab === 'device'
                                    ? 'bg-white dark:bg-base-100 shadow-sm text-blue-600 dark:text-blue-400'
                                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200 hover:bg-gray-200/50 dark:hover:bg-base-300'
                                    }`}
                                onClick={() => setActiveTab('device')}
                            >
                                {t('accounts.add.tabs.device', 'GitHub Login')}
                            </button>
                            <button
                                className={`py-2 px-3 rounded-lg text-sm font-medium transition-all duration-200 ${activeTab === 'token'
                                    ? 'bg-white dark:bg-base-100 shadow-sm text-blue-600 dark:text-blue-400'
                                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200 hover:bg-gray-200/50 dark:hover:bg-base-300'
                                    }`}
                                onClick={() => setActiveTab('token')}
                            >
                                {t('accounts.add.tabs.token_github', 'GitHub Token')}
                            </button>
                        </div>

                        {/* Status Alert */}
                        <StatusAlert />

                        <div className="min-h-[200px]">

                            {/* ── Device Flow Tab ── */}
                            {activeTab === 'device' && (
                                <div className="space-y-5 py-2">

                                    {/* Idle / Error: show start button */}
                                    {(deviceFlow.step === 'idle' || deviceFlow.step === 'error') && (
                                        <div className="text-center space-y-4">
                                            <div className="bg-gray-50 dark:bg-base-200 p-6 rounded-full w-20 h-20 mx-auto flex items-center justify-center">
                                                <Github className="w-10 h-10 text-gray-700 dark:text-gray-300" />
                                            </div>
                                            <div className="space-y-1">
                                                <h4 className="font-medium text-gray-900 dark:text-gray-100">
                                                    {t('accounts.add.device.title', 'Sign in with GitHub')}
                                                </h4>
                                                <p className="text-sm text-gray-500 dark:text-gray-400 max-w-xs mx-auto">
                                                    {t('accounts.add.device.desc', 'Use GitHub Device Flow to securely authorize your account. A code will be shown for you to enter on GitHub.')}
                                                </p>
                                            </div>
                                            <button
                                                className="w-full px-4 py-3 bg-gray-900 dark:bg-white hover:bg-gray-800 dark:hover:bg-gray-100 text-white dark:text-gray-900 font-medium rounded-xl shadow-lg shadow-gray-900/20 dark:shadow-white/10 transition-all flex items-center justify-center gap-2 disabled:opacity-70 disabled:cursor-not-allowed"
                                                onClick={handleStartDeviceFlow}
                                                disabled={deviceFlow.step === 'starting' as any}
                                            >
                                                <Github className="w-5 h-5" />
                                                {t('accounts.add.device.btn_start', 'Start GitHub Login')}
                                            </button>
                                        </div>
                                    )}

                                    {/* Starting: spinner */}
                                    {deviceFlow.step === 'starting' && (
                                        <div className="text-center space-y-4 py-8">
                                            <Loader2 className="w-10 h-10 animate-spin text-blue-500 mx-auto" />
                                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                                {t('accounts.add.device.starting', 'Requesting device code...')}
                                            </p>
                                        </div>
                                    )}

                                    {/* Waiting for user to authorize */}
                                    {deviceFlow.step === 'waiting' && (
                                        <div className="space-y-4">
                                            {/* Step instruction */}
                                            <div className="text-center">
                                                <p className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                                    {t('accounts.add.device.step_enter_code', 'Enter this code on GitHub:')}
                                                </p>
                                            </div>

                                            {/* User Code Display */}
                                            <div
                                                className="relative bg-gray-50 dark:bg-base-200 border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl p-5 text-center cursor-pointer group hover:border-blue-400 dark:hover:border-blue-500 transition-colors"
                                                onClick={() => handleCopyUserCode(deviceFlow.info.user_code)}
                                            >
                                                <code className="text-3xl font-bold tracking-[0.3em] text-gray-900 dark:text-gray-100 select-all">
                                                    {deviceFlow.info.user_code}
                                                </code>
                                                <div className="mt-2 flex items-center justify-center gap-1.5 text-xs text-gray-400 dark:text-gray-500 group-hover:text-blue-500 transition-colors">
                                                    {userCodeCopied ? (
                                                        <>
                                                            <Check className="w-3.5 h-3.5 text-emerald-500" />
                                                            <span className="text-emerald-500">{t('accounts.add.device.copied', 'Copied!')}</span>
                                                        </>
                                                    ) : (
                                                        <>
                                                            <Copy className="w-3.5 h-3.5" />
                                                            <span>{t('accounts.add.device.click_to_copy', 'Click to copy')}</span>
                                                        </>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Open GitHub button */}
                                            <button
                                                className="w-full px-4 py-3 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-xl shadow-lg shadow-blue-500/20 transition-all flex items-center justify-center gap-2"
                                                onClick={() => handleOpenVerification(deviceFlow.info.verification_uri)}
                                            >
                                                <ExternalLink className="w-4 h-4" />
                                                {t('accounts.add.device.btn_open_github', 'Open GitHub Verification Page')}
                                            </button>

                                            {/* Polling status + countdown */}
                                            <div className="flex items-center justify-between text-sm text-gray-500 dark:text-gray-400 px-1">
                                                <div className="flex items-center gap-2">
                                                    <Loader2 className="w-4 h-4 animate-spin text-blue-500" />
                                                    <span>{t('accounts.add.device.waiting', 'Waiting for authorization...')}</span>
                                                </div>
                                                <div className="flex items-center gap-1.5">
                                                    <Timer className="w-3.5 h-3.5" />
                                                    <span className="font-mono text-xs">{formatCountdown(countdown)}</span>
                                                </div>
                                            </div>
                                        </div>
                                    )}

                                    {/* Success */}
                                    {deviceFlow.step === 'success' && (
                                        <div className="text-center space-y-4 py-8">
                                            <CheckCircle2 className="w-12 h-12 text-emerald-500 mx-auto" />
                                            <p className="text-sm font-medium text-emerald-600 dark:text-emerald-400">
                                                {t('accounts.add.device.success', 'Account added successfully!')}
                                            </p>
                                        </div>
                                    )}
                                </div>
                            )}

                            {/* ── GitHub Token Tab ── */}
                            {activeTab === 'token' && (
                                <div className="space-y-4 py-2">
                                    <div className="bg-gray-50 dark:bg-base-200 p-4 rounded-lg border border-gray-200 dark:border-base-300">
                                        <div className="flex justify-between items-center mb-2">
                                            <span className="text-sm font-medium text-gray-500 dark:text-gray-400 flex items-center gap-1.5">
                                                <Key className="w-3.5 h-3.5" />
                                                {t('accounts.add.token.label_github', 'GitHub Token')}
                                            </span>
                                        </div>
                                        <textarea
                                            className="textarea textarea-bordered w-full h-32 font-mono text-xs leading-relaxed focus:outline-none focus:border-blue-500 transition-colors bg-white dark:bg-base-100 text-gray-900 dark:text-base-content border-gray-300 dark:border-base-300 placeholder:text-gray-400"
                                            placeholder={t('accounts.add.token.placeholder_github', 'Paste your GitHub Token (ghp_..., gho_..., ghu_...)\nMultiple tokens can be separated by new lines.')}
                                            value={githubToken}
                                            onChange={(e) => setGithubToken(e.target.value)}
                                            disabled={status === 'loading' || status === 'success'}
                                        />
                                        <p className="text-[10px] text-gray-400 mt-2">
                                            {t('accounts.add.token.hint_github', 'Supports GitHub Personal Access Tokens (ghp_), OAuth tokens (gho_), and user-to-server tokens (ghu_). You can paste multiple tokens, one per line.')}
                                        </p>
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* Footer buttons */}
                        <div className="flex gap-3 w-full mt-6">
                            <button
                                className="flex-1 px-4 py-2.5 bg-gray-100 dark:bg-base-200 text-gray-700 dark:text-gray-300 font-medium rounded-xl hover:bg-gray-200 dark:hover:bg-base-300 transition-colors focus:outline-none focus:ring-2 focus:ring-200 dark:focus:ring-base-300"
                                onClick={async () => {
                                    if (deviceFlow.step === 'waiting') {
                                        await handleCancelDeviceFlow();
                                    }
                                    setIsOpen(false);
                                }}
                                disabled={status === 'success'}
                            >
                                {deviceFlow.step === 'waiting'
                                    ? t('accounts.add.btn_cancel_flow', 'Cancel')
                                    : t('accounts.add.btn_cancel')
                                }
                            </button>
                            {activeTab === 'token' && (
                                <button
                                    className="flex-1 px-4 py-2.5 text-white font-medium rounded-xl shadow-md transition-all focus:outline-none focus:ring-2 focus:ring-offset-2 bg-blue-500 hover:bg-blue-600 focus:ring-blue-500 shadow-blue-100 dark:shadow-blue-900/30 flex justify-center items-center gap-2"
                                    onClick={handleTokenSubmit}
                                    disabled={status === 'loading' || status === 'success'}
                                >
                                    {status === 'loading' ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
                                    {t('accounts.add.btn_confirm')}
                                </button>
                            )}
                        </div>
                    </div>
                </div>,
                document.body
            )}
        </>
    );
}

export default AddAccountDialog;
