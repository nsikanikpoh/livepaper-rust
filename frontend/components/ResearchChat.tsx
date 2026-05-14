'use client';
import { useState, useRef, useEffect } from 'react';
import { Send, BookOpen, User, Square } from 'lucide-react';
import { useApiClient } from '@/lib/api';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface Message {
    id: string;
    role: 'user' | 'assistant';
    content: string;
    timestamp: Date;
}

interface ChatApiResponse {
    session_id: string;
    message: string;
    sources: unknown;
    escalated: boolean;
    escalation_note?: string;
    trace_id: string;
}

// Markdown renderer styled to match the LivePaper aesthetic.
// Each element is mapped to a component so prose, lists, code blocks,
// tables, and blockquotes all render consistently without any global CSS.
function MarkdownContent({ content }: { content: string }) {
    return (
        <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
                p: ({ children }) => (
                    <p className="mb-3 last:mb-0 leading-relaxed text-[15px] text-[#2c2217]">
                        {children}
                    </p>
                ),
                h1: ({ children }) => (
                    <h1 className="text-lg font-bold text-[#2c2217] mt-5 mb-2 first:mt-0" style={{ fontFamily: "'Georgia', serif" }}>
                        {children}
                    </h1>
                ),
                h2: ({ children }) => (
                    <h2 className="text-base font-bold text-[#2c2217] mt-4 mb-2 first:mt-0" style={{ fontFamily: "'Georgia', serif" }}>
                        {children}
                    </h2>
                ),
                h3: ({ children }) => (
                    <h3 className="text-[15px] font-semibold text-[#2c2217] mt-3 mb-1.5 first:mt-0" style={{ fontFamily: "'Georgia', serif" }}>
                        {children}
                    </h3>
                ),
                ul: ({ children }) => (
                    <ul className="mb-3 space-y-1.5 pl-1" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        {children}
                    </ul>
                ),
                ol: ({ children }) => (
                    <ol className="mb-3 space-y-1.5 pl-1" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        {children}
                    </ol>
                ),
                li: ({ children, ordered, index }: any) => (
                    <li className="flex gap-2.5 text-[14px] text-[#2c2217] leading-relaxed">
                        <span className="shrink-0 mt-[3px] text-[#6b1f2a] font-bold select-none">
                            {ordered ? `${(index ?? 0) + 1}.` : '·'}
                        </span>
                        <span className="flex-1">{children}</span>
                    </li>
                ),
                code: ({ inline, children }: any) =>
                    inline ? (
                        <code className="px-1.5 py-0.5 rounded bg-[#f0e8e0] text-[#6b1f2a] text-[13px] font-mono">
                            {children}
                        </code>
                    ) : (
                        <code className="block text-[13px] font-mono text-[#2c2217]">
                            {children}
                        </code>
                    ),
                pre: ({ children }) => (
                    <pre className="mb-3 p-4 rounded-xl bg-[#f0e8e0] overflow-x-auto text-[13px] font-mono text-[#2c2217] leading-relaxed">
                        {children}
                    </pre>
                ),
                blockquote: ({ children }) => (
                    <blockquote className="mb-3 pl-4 border-l-2 border-[#6b1f2a] text-[#5a4535] italic text-[14px] leading-relaxed" style={{ fontFamily: "'Georgia', serif" }}>
                        {children}
                    </blockquote>
                ),
                strong: ({ children }) => (
                    <strong className="font-semibold text-[#2c2217]">{children}</strong>
                ),
                em: ({ children }) => (
                    <em className="italic text-[#5a4535]">{children}</em>
                ),
                hr: () => (
                    <hr className="my-4 border-[#e0d8cf]" />
                ),
                a: ({ href, children }) => (
                    <a
                        href={href}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-[#6b1f2a] underline underline-offset-2 hover:text-[#b8860b] transition-colors duration-150"
                    >
                        {children}
                    </a>
                ),
                table: ({ children }) => (
                    <div className="mb-3 overflow-x-auto rounded-xl border border-[#e0d8cf]">
                        <table className="w-full text-[13px]" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            {children}
                        </table>
                    </div>
                ),
                thead: ({ children }) => (
                    <thead className="bg-[#faf7f4] border-b border-[#e0d8cf]">{children}</thead>
                ),
                th: ({ children }) => (
                    <th className="px-4 py-2.5 text-left font-semibold text-[#5a4535] text-[12px] uppercase tracking-wide">
                        {children}
                    </th>
                ),
                td: ({ children }) => (
                    <td className="px-4 py-2.5 text-[#2c2217] border-t border-[#f0e8e0]">
                        {children}
                    </td>
                ),
            }}
        >
            {content}
        </ReactMarkdown>
    );
}

export default function ResearchChat() {
    const api = useApiClient();

    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [sessionId, setSessionId] = useState<string>('');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const sendMessage = async () => {
        if (!input.trim() || isLoading) return;

        const userMessage: Message = {
            id: Date.now().toString(),
            role: 'user',
            content: input,
            timestamp: new Date(),
        };

        setMessages(prev => [...prev, userMessage]);
        setInput('');
        if (inputRef.current) inputRef.current.style.height = 'auto';
        setIsLoading(true);

        try {
            const data = await api.post<ChatApiResponse>('/chat', {
                message: userMessage.content,
                session_id: sessionId || undefined,
            });

            if (!sessionId) setSessionId(data.session_id);

            // Show escalation note as a blockquote beneath the main answer
            const content = data.escalation_note
                ? `${data.message}\n\n> ${data.escalation_note}`
                : data.message;

            setMessages(prev => [...prev, {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content,
                timestamp: new Date(),
            }]);
        } catch {
            setMessages(prev => [...prev, {
                id: (Date.now() + 1).toString(),
                role: 'assistant',
                content: 'Sorry, I encountered an error. Please try again.',
                timestamp: new Date(),
            }]);
        } finally {
            setIsLoading(false);
            setTimeout(() => inputRef.current?.focus(), 100);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            sendMessage();
        }
    };

    const isEmpty = messages.length === 0;

    return (
        <div className="flex flex-col h-full bg-[#f5f0eb]" style={{ fontFamily: "'Georgia', 'Times New Roman', serif" }}>

            <div className="flex-1 overflow-y-auto">
                {isEmpty ? (
                    <div className="flex flex-col items-center justify-center h-full px-4 pt-6">
                        <div className="mb-6">
                            <div className="w-14 h-14 rounded-full bg-[#6b1f2a] flex items-center justify-center shadow-sm">
                                <BookOpen className="w-7 h-7 text-white" />
                            </div>
                        </div>
                        <h1 className="text-2xl font-semibold text-[#2c2217] mb-2 text-center">
                            What would you like to explore?
                        </h1>
                        <p className="text-[#8a7060] text-center max-w-sm text-sm leading-relaxed" style={{ fontFamily: 'system-ui, sans-serif' }}>
                            Ask about research papers, request author connections, or dive deep into any academic topic.
                        </p>

                        <div className="mt-8 flex flex-wrap gap-2 justify-center max-w-lg">
                            {[
                                'Summarize recent papers on transformer architectures',
                                'Find papers about climate change mitigation',
                                'Connect me with authors researching CRISPR',
                                'What are the latest findings in quantum computing?',
                            ].map((suggestion) => (
                                <button
                                    key={suggestion}
                                    onClick={() => { setInput(suggestion); inputRef.current?.focus(); }}
                                    className="px-3 py-2 text-xs rounded-lg border border-[#d4c8bc] bg-white text-[#5a4535] hover:bg-[#ede6dc] hover:border-[#6b1f2a] transition-all duration-150 text-left leading-snug"
                                    style={{ fontFamily: 'system-ui, sans-serif' }}
                                >
                                    {suggestion}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    <div className="max-w-3xl mx-auto w-full px-4 py-6 space-y-6">
                        {messages.map((message) => (
                            <div
                                key={message.id}
                                className={`flex gap-3 ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}
                            >
                                {message.role === 'assistant' && (
                                    <div className="flex-shrink-0 mt-1">
                                        <div className="w-8 h-8 rounded-full bg-[#6b1f2a] flex items-center justify-center shadow-sm">
                                            <BookOpen className="w-4 h-4 text-white" />
                                        </div>
                                    </div>
                                )}

                                <div className={`group relative max-w-[80%] ${message.role === 'user' ? 'items-end' : 'items-start'} flex flex-col`}>
                                    {message.role === 'user' ? (
                                        <div className="bg-[#2c2217] text-[#f5f0eb] px-4 py-3 rounded-2xl rounded-br-sm text-[15px] leading-relaxed whitespace-pre-wrap">
                                            {message.content}
                                        </div>
                                    ) : (
                                        <div className="min-w-0">
                                            <MarkdownContent content={message.content} />
                                        </div>
                                    )}
                                    <span
                                        className="text-[10px] text-[#b0a090] mt-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200 px-1"
                                        style={{ fontFamily: 'system-ui, sans-serif' }}
                                    >
                                        {message.timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                    </span>
                                </div>

                                {message.role === 'user' && (
                                    <div className="flex-shrink-0 mt-1">
                                        <div className="w-8 h-8 rounded-full bg-[#8a7060] flex items-center justify-center">
                                            <User className="w-4 h-4 text-white" />
                                        </div>
                                    </div>
                                )}
                            </div>
                        ))}

                        {isLoading && (
                            <div className="flex gap-3 justify-start">
                                <div className="flex-shrink-0 mt-1">
                                    <div className="w-8 h-8 rounded-full bg-[#6b1f2a] flex items-center justify-center">
                                        <BookOpen className="w-4 h-4 text-white" />
                                    </div>
                                </div>
                                <div className="flex items-center gap-1 px-1 py-3">
                                    <span className="w-1.5 h-1.5 rounded-full bg-[#6b1f2a] animate-bounce" style={{ animationDelay: '0ms' }} />
                                    <span className="w-1.5 h-1.5 rounded-full bg-[#6b1f2a] animate-bounce" style={{ animationDelay: '150ms' }} />
                                    <span className="w-1.5 h-1.5 rounded-full bg-[#6b1f2a] animate-bounce" style={{ animationDelay: '300ms' }} />
                                </div>
                            </div>
                        )}

                        <div ref={messagesEndRef} />
                    </div>
                )}
            </div>

            <div className={`px-4 pb-5 pt-3 ${isEmpty ? '' : 'border-t border-[#e0d8cf] bg-[#f5f0eb]'}`}>
                <div className="max-w-3xl mx-auto">
                    <div className="relative bg-white rounded-2xl border border-[#d4c8bc] shadow-sm focus-within:border-[#6b1f2a] focus-within:shadow-md transition-all duration-200">
                        <textarea
                            ref={inputRef}
                            value={input}
                            onChange={(e) => {
                                setInput(e.target.value);
                                e.target.style.height = 'auto';
                                e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
                            }}
                            onKeyDown={handleKeyDown}
                            placeholder="Ask about research papers, authors, or topics…"
                            rows={1}
                            disabled={isLoading}
                            autoFocus
                            className="w-full px-4 pt-3 pb-12 text-[15px] text-[#2c2217] placeholder-[#b0a090] bg-transparent resize-none focus:outline-none min-h-[52px] max-h-[200px] overflow-y-auto leading-relaxed"
                            style={{ fontFamily: "'Georgia', serif" }}
                        />
                        <div className="absolute bottom-2 left-3 right-3 flex items-center justify-between">
                            <span className="text-[11px] text-[#c0b0a0]" style={{ fontFamily: 'system-ui, sans-serif' }}>
                                Shift+Enter for new line
                            </span>
                            <button
                                onClick={sendMessage}
                                disabled={!input.trim() || isLoading}
                                className="w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-150 disabled:opacity-30 disabled:cursor-not-allowed bg-[#6b1f2a] hover:bg-[#b8860b] active:scale-95"
                            >
                                {isLoading
                                    ? <Square className="w-3.5 h-3.5 text-white fill-white" />
                                    : <Send className="w-3.5 h-3.5 text-white" />}
                            </button>
                        </div>
                    </div>
                    <p className="text-center text-[11px] text-[#c0b0a0] mt-2" style={{ fontFamily: 'system-ui, sans-serif' }}>
                        LivePaper may make mistakes. Verify important research details.
                    </p>
                </div>
            </div>
        </div>
    );
}